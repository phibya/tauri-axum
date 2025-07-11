use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::middleware::AuthenticatedUser;
use crate::database::{
    models::{
        CreateModelProviderRequest, CreateModelRequest, ModelProvider, ModelProviderListResponse,
        ModelProviderModel, TestModelProviderProxyRequest, TestModelProviderProxyResponse,
        UpdateModelProviderRequest, UpdateModelRequest, UserGroup,
    },
    queries::{model_providers, user_group_model_providers},
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

// Model Provider endpoints
pub async fn list_model_providers(
    Extension(auth_user): Extension<AuthenticatedUser>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<ModelProviderListResponse>, StatusCode> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    // Get model providers based on user permissions
    let user_providers =
        match user_group_model_providers::get_model_providers_for_user(auth_user.user.id).await {
            Ok(providers) => providers,
            Err(e) => {
                eprintln!(
                    "Failed to get model providers for user {}: {}",
                    auth_user.user.id, e
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

    // Calculate pagination
    let total = user_providers.len() as i64;
    let start = ((page - 1) * per_page) as usize;
    let end = (start + per_page as usize).min(user_providers.len());

    let paginated_providers = if start < user_providers.len() {
        user_providers[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(Json(ModelProviderListResponse {
        providers: paginated_providers,
        total,
        page,
        per_page,
    }))
}

pub async fn get_model_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<ModelProvider>, StatusCode> {
    match model_providers::get_model_provider_by_id(provider_id).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to get model provider {}: {}", provider_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_model_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Json(mut request): Json<CreateModelProviderRequest>,
) -> Result<Json<ModelProvider>, StatusCode> {
    // Validate provider type
    let valid_types = [
        "llama.cpp",
        "openai",
        "anthropic",
        "groq",
        "gemini",
        "mistral",
        "custom",
    ];
    if !valid_types.contains(&request.provider_type.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate requirements for enabling non-llama.cpp providers
    if let Some(true) = request.enabled {
        if request.provider_type != "llama.cpp" {
            // Check API key
            if request.api_key.is_none() || request.api_key.as_ref().unwrap().trim().is_empty() {
                eprintln!("Cannot create enabled provider: API key is required");
                return Err(StatusCode::BAD_REQUEST);
            }

            // Check base URL
            if request.base_url.is_none() || request.base_url.as_ref().unwrap().trim().is_empty() {
                eprintln!("Cannot create enabled provider: Base URL is required");
                return Err(StatusCode::BAD_REQUEST);
            }

            // Validate URL format
            if !is_valid_url(request.base_url.as_ref().unwrap()) {
                eprintln!("Cannot create enabled provider: Invalid base URL format");
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            // Llama.cpp providers must start disabled (no models yet)
            request.enabled = Some(false);
        }
    }

    match model_providers::create_model_provider(request).await {
        Ok(provider) => Ok(Json(provider)),
        Err(e) => {
            eprintln!("Failed to create model provider: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn is_valid_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok()
}

pub async fn update_model_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
    Json(request): Json<UpdateModelProviderRequest>,
) -> Result<Json<ModelProvider>, StatusCode> {
    // If trying to enable the provider, validate requirements
    if let Some(true) = request.enabled {
        // Get the current provider to check its state
        match model_providers::get_model_provider_by_id(provider_id).await {
            Ok(Some(current_provider)) => {
                // Check if provider type requires API key and base URL
                if current_provider.provider_type != "llama.cpp" {
                    // Check API key
                    let api_key = request
                        .api_key
                        .as_ref()
                        .or(current_provider.api_key.as_ref());
                    if api_key.is_none() || api_key.unwrap().trim().is_empty() {
                        eprintln!(
                            "Cannot enable provider {}: API key is required",
                            provider_id
                        );
                        return Err(StatusCode::BAD_REQUEST);
                    }

                    // Check base URL
                    let base_url = request
                        .base_url
                        .as_ref()
                        .or(current_provider.base_url.as_ref());
                    if base_url.is_none() || base_url.unwrap().trim().is_empty() {
                        eprintln!(
                            "Cannot enable provider {}: Base URL is required",
                            provider_id
                        );
                        return Err(StatusCode::BAD_REQUEST);
                    }

                    // Validate URL format
                    if !is_valid_url(base_url.unwrap()) {
                        eprintln!(
                            "Cannot enable provider {}: Invalid base URL format",
                            provider_id
                        );
                        return Err(StatusCode::BAD_REQUEST);
                    }
                }

                // Check if provider has any models
                if current_provider.models.is_empty() {
                    eprintln!(
                        "Cannot enable provider {}: No models available",
                        provider_id
                    );
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
            Ok(None) => return Err(StatusCode::NOT_FOUND),
            Err(e) => {
                eprintln!("Failed to get model provider {}: {}", provider_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    match model_providers::update_model_provider(provider_id, request).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to update model provider {}: {}", provider_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_model_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match model_providers::delete_model_provider(provider_id).await {
        Ok(Ok(true)) => Ok(StatusCode::NO_CONTENT),
        Ok(Ok(false)) => Err(StatusCode::NOT_FOUND),
        Ok(Err(error_message)) => {
            eprintln!(
                "Cannot delete model provider {}: {}",
                provider_id, error_message
            );
            // Return a JSON response with the error message for better UX
            Err(StatusCode::BAD_REQUEST)
        }
        Err(e) => {
            eprintln!("Failed to delete model provider {}: {}", provider_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn clone_model_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<ModelProvider>, StatusCode> {
    match model_providers::clone_model_provider(provider_id).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to clone model provider {}: {}", provider_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Model endpoints
pub async fn create_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
    Json(request): Json<CreateModelRequest>,
) -> Result<Json<ModelProviderModel>, StatusCode> {
    match model_providers::create_model(provider_id, request).await {
        Ok(model) => Ok(Json(model)),
        Err(e) => {
            eprintln!("Failed to create model for provider {}: {}", provider_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn update_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<UpdateModelRequest>,
) -> Result<Json<ModelProviderModel>, StatusCode> {
    match model_providers::update_model(model_id, request).await {
        Ok(Some(model)) => Ok(Json(model)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to update model {}: {}", model_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match model_providers::delete_model(model_id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to delete model {}: {}", model_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> Result<Json<ModelProviderModel>, StatusCode> {
    match model_providers::get_model_by_id(model_id).await {
        Ok(Some(model)) => Ok(Json(model)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("Failed to get model {}: {}", model_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Test proxy connection for model provider
pub async fn test_model_provider_proxy_connection(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(_provider_id): Path<Uuid>,
    Json(request): Json<TestModelProviderProxyRequest>,
) -> Result<Json<TestModelProviderProxyResponse>, StatusCode> {
    // Test the proxy connection by making a simple HTTP request through the proxy
    match test_proxy_connectivity_for_provider(&request).await {
        Ok(()) => Ok(Json(TestModelProviderProxyResponse {
            success: true,
            message: "Proxy connection successful".to_string(),
        })),
        Err(e) => Ok(Json(TestModelProviderProxyResponse {
            success: false,
            message: format!("Proxy connection failed: {}", e),
        })),
    }
}

async fn test_proxy_connectivity_for_provider(
    proxy_config: &TestModelProviderProxyRequest,
) -> Result<(), String> {
    // Validate proxy URL format
    if proxy_config.url.trim().is_empty() {
        return Err("Proxy URL is empty".to_string());
    }

    // Parse and validate the proxy URL
    let _proxy_url = reqwest::Url::parse(&proxy_config.url)
        .map_err(|e| format!("Invalid proxy URL format: {}", e))?;

    // Create a reqwest client with proxy configuration
    let mut proxy_builder = reqwest::Proxy::all(&proxy_config.url)
        .map_err(|e| format!("Failed to create proxy: {}", e))?;

    // Add authentication if provided
    if !proxy_config.username.is_empty() {
        proxy_builder = proxy_builder.basic_auth(&proxy_config.username, &proxy_config.password);
    }

    // Build the client with proxy and SSL settings
    let mut client_builder = reqwest::Client::builder()
        .proxy(proxy_builder)
        .timeout(std::time::Duration::from_secs(30)) // Increased timeout for proxy connections
        .no_proxy(); // Disable system proxy to ensure we only use our configured proxy

    // Configure SSL verification based on settings
    if proxy_config.ignore_ssl_certificates {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Test the proxy by making a request to a reliable endpoint
    // Using httpbin.org as it's a simple testing service that returns IP info
    let test_url = "https://httpbin.org/ip";

    match client.get(test_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Try to read the response to ensure it's valid
                match response.text().await {
                    Ok(body) => {
                        // Verify the response contains expected IP information
                        if body.contains("origin") {
                            Ok(())
                        } else {
                            Err(format!("Unexpected response format: {}", body))
                        }
                    }
                    Err(e) => Err(format!("Failed to read response body: {}", e)),
                }
            } else {
                Err(format!(
                    "HTTP request failed with status: {}",
                    response.status()
                ))
            }
        }
        Err(e) => {
            // Check if it's a proxy-related error
            let error_msg = e.to_string();
            if error_msg.contains("proxy") || error_msg.contains("CONNECT") {
                Err(format!("Proxy connection failed: {}", e))
            } else if error_msg.contains("timeout") {
                Err("Proxy connection timed out".to_string())
            } else if error_msg.contains("dns") {
                Err(format!(
                    "DNS resolution failed (check proxy settings): {}",
                    e
                ))
            } else {
                Err(format!("Network request failed: {}", e))
            }
        }
    }
}

// Get groups that have access to a model provider
pub async fn get_provider_groups(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<Vec<UserGroup>>, StatusCode> {
    match user_group_model_providers::get_groups_for_model_provider(provider_id).await {
        Ok(groups) => Ok(Json(groups)),
        Err(e) => {
            eprintln!("Error getting groups for model provider: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
