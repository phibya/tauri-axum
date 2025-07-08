mod api;
mod auth;
mod database;

use crate::api::app::methods::get_http_port;
use axum::{body::Body, http::Request, response::Response, routing::get, routing::post, Router};
use once_cell::sync::Lazy;
use std::net::SocketAddr;
use std::path::PathBuf;
use tauri::webview::WebviewWindowBuilder;
use tokio::signal;
use tower_http::cors::CorsLayer;

pub static APP_NAME: Lazy<String> =
    Lazy::new(|| std::env::var("APP_NAME").unwrap_or_else(|_| "ziee".to_string()));
pub static APP_DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    std::env::var("APP_DATA_DIR")
        .unwrap_or_else(|_| {
            // {homedir}/.ziee
            let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            home_dir
                .join(".ziee")
                .to_str()
                .unwrap_or_default()
                .to_string()
        })
        .parse()
        .unwrap()
});
pub static HTTP_PORT: Lazy<u16> = Lazy::new(|| get_available_port());

pub fn run() {
    let port = get_http_port();

    if std::env::var("HEADLESS").unwrap_or_default() == "true" {
        // Headless mode: Run server only without Tauri GUI
        println!("Starting headless API server on port: {}", port);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = database::initialize_database().await {
                eprintln!("Failed to initialize database: {}", e);
                std::process::exit(1);
            }

            let api_router = create_rest_router();

            // Setup graceful shutdown
            let (tx, rx) = tokio::sync::oneshot::channel();

            // Spawn the server task
            let server_task = tokio::spawn(async move {
                start_api_server(port, api_router).await;
            });

            // Setup signal handler
            tokio::spawn(async move {
                shutdown_signal().await;
                let _ = tx.send(());
            });

            // Wait for shutdown signal
            let _ = rx.await;

            // Cleanup database
            database::cleanup_database().await;

            server_task.abort();
            println!("Application shutdown complete");
        });
    } else {
        // GUI mode: Run with Tauri
        println!("Starting Tauri application with API on port: {}", port);

        tauri::Builder::default()
            .invoke_handler(tauri::generate_handler![get_http_port,])
            .setup(move |app| {
                // Create the API router
                let api_router = create_rest_router();

                // Initialize database
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = database::initialize_database().await {
                        eprintln!("Failed to initialize database: {}", e);
                    }
                });

                // Register the API router with the Tauri application
                tauri::async_runtime::spawn(async move {
                    start_api_server(port, api_router).await;
                });

                // Production mode: open default Tauri webview without binding port
                println!("Production mode: Opening default Tauri webview");

                WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                    .title("React Test App")
                    .inner_size(800.0, 600.0)
                    .build()?;

                Ok(())
            })
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    // Cleanup database before closing
                    tauri::async_runtime::spawn(async move {
                        database::cleanup_database().await;
                    });
                }
            })
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}

async fn start_api_server(port: u16, api_router: Router) {
    let app = if cfg!(debug_assertions) {
        // Development: Proxy non-API routes to Vite dev server
        println!(
            "Development mode: API server with proxy to Vite on port {}",
            port
        );
        api_router
            .layer(CorsLayer::permissive())
            .fallback(proxy_to_vite)
    } else if std::env::var("HEADLESS").unwrap_or_default() == "true" {
        // Headless mode: Serve UI folder if it exists
        println!("Headless mode: API + Frontend server on port {}", port);
        use tower_http::services::ServeDir;
        let static_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("ui");

        if static_dir.exists() {
            println!("Serving UI from: {}", static_dir.display());
            api_router
                .layer(CorsLayer::permissive())
                .fallback_service(ServeDir::new(static_dir))
        } else {
            println!(
                "Warning: UI folder not found at {}, serving API only",
                static_dir.display()
            );
            api_router.layer(CorsLayer::permissive())
        }
    } else {
        // Production mode: API only (webview handles frontend)
        println!("Production mode: API server only on port {}", port);
        api_router.layer(CorsLayer::permissive())
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("API server error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to bind to port {}: {}", port, e);
        }
    }
}

// Proxy handler to forward requests to Vite dev server
async fn proxy_to_vite(req: Request<Body>) -> Result<Response<Body>, axum::http::StatusCode> {
    let vite_url =
        std::env::var("TAURI_DEV_HOST").unwrap_or_else(|_| "http://localhost:1420".to_string());
    let uri = req.uri();
    let path_and_query = uri
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or(uri.path());

    let proxy_url = format!("{}{}", vite_url, path_and_query);

    // Create a new HTTP client request
    match reqwest::get(&proxy_url).await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes()
                .await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

            let mut builder = Response::builder().status(status);

            // Copy headers properly
            for (key, value) in headers.iter() {
                builder = builder.header(key.as_str(), value);
            }

            builder
                .body(Body::from(body))
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(_) => Err(axum::http::StatusCode::BAD_GATEWAY),
    }
}

pub fn get_available_port() -> u16 {
    // Try PORT environment variable first
    if let Ok(port_str) = std::env::var("PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            return port;
        }
    }

    // Try default port 1430
    if std::net::TcpListener::bind("127.0.0.1:1430").is_ok() {
        return 1430;
    }

    // Use portpicker to find a random available port
    portpicker::pick_unused_port().unwrap_or(3000)
}

fn create_rest_router() -> Router {
    Router::new()
        .route("/api/user/greet", post(api::user::methods::greet))
        .route("/api/auth/init", get(api::auth::methods::check_init_status))
        .route("/api/auth/setup", post(api::auth::methods::init_app))
        .route("/api/auth/login", post(api::auth::methods::login))
        .route("/api/auth/logout", post(api::auth::methods::logout))
        .route("/api/auth/register", post(api::auth::methods::register))
        .route("/api/auth/me", get(api::auth::methods::me))
        .route("/health", get(|| async { "Tauri + Localhost Plugin OK" }))
        .layer(CorsLayer::permissive())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            println!("Received Ctrl+C, shutting down...");
        },
        _ = terminate => {
            println!("Received terminate signal, shutting down...");
        },
    }
}
