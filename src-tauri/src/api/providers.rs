use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::errors::{ApiResult, AppError, ErrorCode};
use crate::api::middleware::AuthenticatedUser;
use crate::database::{
    models::{
        AvailableDevicesResponse, CreateModelRequest, CreateProviderRequest, Model, Provider,
        ProviderListResponse, ProviderProxySettings, TestProviderProxyResponse, UpdateModelRequest,
        UpdateProviderRequest, UserGroup,
    },
    queries::{providers, user_group_providers},
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

// Provider endpoints
pub async fn list_providers(
    Extension(auth_user): Extension<AuthenticatedUser>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<ProviderListResponse>> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    // Get providers based on user permissions
    let user_providers = match user_group_providers::get_providers_for_user(auth_user.user.id).await
    {
        Ok(providers) => providers,
        Err(e) => {
            eprintln!(
                "Failed to get model providers for user {}: {}",
                auth_user.user.id, e
            );
            return Err(e.into());
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

    Ok(Json(ProviderListResponse {
        providers: paginated_providers,
        total,
        page,
        per_page,
    }))
}

pub async fn get_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<Provider>> {
    match providers::get_provider_by_id(provider_id).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to get model provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

pub async fn create_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Json(mut request): Json<CreateProviderRequest>,
) -> ApiResult<Json<Provider>> {
    // Validate provider type
    let valid_types = [
        "candle",
        "openai",
        "anthropic",
        "groq",
        "gemini",
        "mistral",
        "custom",
    ];
    if !valid_types.contains(&request.provider_type.as_str()) {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Invalid request",
        ));
    }

    // Validate requirements for enabling non-candle_server providers
    if let Some(true) = request.enabled {
        if request.provider_type != "candle" {
            // Check API key
            if request.api_key.is_none() || request.api_key.as_ref().unwrap().trim().is_empty() {
                eprintln!("Cannot create enabled provider: API key is required");
                return Err(AppError::new(
                    crate::api::errors::ErrorCode::ValidInvalidInput,
                    "Invalid request",
                ));
            }

            // Check base URL
            if request.base_url.is_none() || request.base_url.as_ref().unwrap().trim().is_empty() {
                eprintln!("Cannot create enabled provider: Base URL is required");
                return Err(AppError::new(
                    crate::api::errors::ErrorCode::ValidInvalidInput,
                    "Invalid request",
                ));
            }

            // Validate URL format
            if !is_valid_url(request.base_url.as_ref().unwrap()) {
                eprintln!("Cannot create enabled provider: Invalid base URL format");
                return Err(AppError::new(
                    crate::api::errors::ErrorCode::ValidInvalidInput,
                    "Invalid request",
                ));
            }
        } else {
            // Llama.cpp providers must start disabled (no models yet)
            request.enabled = Some(false);
        }
    }

    match providers::create_provider(request).await {
        Ok(provider) => Ok(Json(provider)),
        Err(e) => {
            eprintln!("Failed to create model provider: {}", e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

fn is_valid_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok()
}

pub async fn update_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
    Json(request): Json<UpdateProviderRequest>,
) -> ApiResult<Json<Provider>> {
    // If trying to enable the provider, validate requirements
    if let Some(true) = request.enabled {
        // Get the current provider to check its state
        match providers::get_provider_by_id(provider_id).await {
            Ok(Some(current_provider)) => {
                // Check if provider type requires API key and base URL
                if current_provider.provider_type != "candle" {
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
                        return Err(AppError::new(
                            crate::api::errors::ErrorCode::ValidInvalidInput,
                            "Invalid operation",
                        ));
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
                        return Err(AppError::new(
                            crate::api::errors::ErrorCode::ValidInvalidInput,
                            "Invalid operation",
                        ));
                    }

                    // Validate URL format
                    if !is_valid_url(base_url.unwrap()) {
                        eprintln!(
                            "Cannot enable provider {}: Invalid base URL format",
                            provider_id
                        );
                        return Err(AppError::new(
                            crate::api::errors::ErrorCode::ValidInvalidInput,
                            "Invalid operation",
                        ));
                    }
                }

                // Check if provider has any models
                let provider_models = match providers::get_models_for_provider(provider_id).await {
                    Ok(models) => models,
                    Err(e) => {
                        eprintln!(
                            "Error fetching models for provider {}: {:?}",
                            provider_id, e
                        );
                        return Err(AppError::from(e));
                    }
                };

                if provider_models.is_empty() {
                    eprintln!(
                        "Cannot enable provider {}: No models available",
                        provider_id
                    );
                    return Err(AppError::new(
                        crate::api::errors::ErrorCode::ValidInvalidInput,
                        "Invalid operation",
                    ));
                }
            }
            Ok(None) => return Err(AppError::not_found("Resource")),
            Err(e) => {
                eprintln!("Failed to get model provider {}: {}", provider_id, e);
                return Err(e.into());
            }
        }
    }

    match providers::update_provider(provider_id, request).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to update model provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

pub async fn delete_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    match providers::delete_provider(provider_id).await {
        Ok(Ok(true)) => Ok(StatusCode::NO_CONTENT),
        Ok(Ok(false)) => Err(AppError::not_found("Resource")),
        Ok(Err(error_message)) => {
            eprintln!(
                "Cannot delete model provider {}: {}",
                provider_id, error_message
            );
            // Return a JSON response with the error message for better UX
            Err(AppError::new(
                crate::api::errors::ErrorCode::ValidInvalidInput,
                "Cannot delete model provider",
            ))
        }
        Err(e) => {
            eprintln!("Failed to delete model provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

pub async fn clone_provider(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<Provider>> {
    match providers::clone_provider(provider_id).await {
        Ok(Some(provider)) => Ok(Json(provider)),
        Ok(None) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to clone model provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

// Model endpoints
pub async fn create_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
    Json(request): Json<CreateModelRequest>,
) -> ApiResult<Json<Model>> {
    match providers::create_model(provider_id, request).await {
        Ok(model) => Ok(Json(model)),
        Err(e) => {
            eprintln!("Failed to create model for provider {}: {}", provider_id, e);
            // Handle unique constraint violation for (provider_id, name)
            match &e {
        sqlx::Error::Database(db_err) if db_err.constraint() == Some("models_provider_id_name_unique") => {
          Err(AppError::new(ErrorCode::ValidInvalidInput,
                            "A model with this ID already exists for this provider. Please use a different model ID."))
        }
        _ => Err(AppError::internal_error("Database operation failed"))
      }
        }
    }
}

pub async fn update_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<UpdateModelRequest>,
) -> ApiResult<Json<Model>> {
    match providers::update_model(model_id, request).await {
        Ok(Some(model)) => Ok(Json(model)),
        Ok(None) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to update model {}: {}", model_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

pub async fn delete_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Get the model database record using proper database query
    let model = match providers::get_model_db_by_id(model_id).await {
        Ok(Some(model)) => model,
        Ok(None) => return Err(AppError::not_found("Model")),
        Err(e) => {
            eprintln!("Failed to get model {}: {}", model_id, e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    // Get the provider to check if it's a Candle provider
    let provider = match providers::get_provider_by_id(model.provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => return Err(AppError::not_found("Model provider")),
        Err(e) => {
            eprintln!("Failed to get model provider: {}", e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    // If it's a Candle provider, handle model shutdown and file deletion
    if provider.provider_type == "candle" {
        // First, stop the model if it's running
        let model_manager = crate::ai::candle_server::management::get_model_manager();
        let model_path = model.get_model_path();

        println!(
            "Checking and cleaning up model {} before deletion",
            model_id
        );
        match model_manager.check_and_cleanup_model(&model_path).await {
            Ok(()) => {
                println!("Successfully cleaned up model {} for deletion", model_id);
            }
            Err(e) => {
                eprintln!("Warning: Failed to cleanup model {}: {}", model_id, e);
                // Continue with deletion anyway
            }
        }

        // Delete the physical model files
        let model_path = model.get_model_path();
        let full_model_path = crate::APP_DATA_DIR.join(&model_path);

        println!(
            "Deleting Candle model files at: {}",
            full_model_path.display()
        );

        if full_model_path.exists() {
            match std::fs::remove_dir_all(&full_model_path) {
                Ok(()) => {
                    println!("Successfully deleted model files for model {}", model_id);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to delete model files for model {}: {}",
                        model_id, e
                    );
                    // Continue with database deletion even if file deletion fails
                    // This prevents orphaned database records
                }
            }
        } else {
            println!(
                "Model files not found at {}, skipping file deletion",
                full_model_path.display()
            );
        }
    }

    // Delete the model from the database
    match providers::delete_model(model_id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to delete model {} from database: {}", model_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

pub async fn get_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<Model>> {
    match providers::get_model_by_id(model_id).await {
        Ok(Some(model)) => Ok(Json(model)),
        Ok(None) => Err(AppError::not_found("Resource")),
        Err(e) => {
            eprintln!("Failed to get model {}: {}", model_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

// Test proxy connection for model provider
pub async fn test_provider_proxy_connection(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(_provider_id): Path<Uuid>,
    Json(request): Json<ProviderProxySettings>,
) -> ApiResult<Json<TestProviderProxyResponse>> {
    // Test the proxy connection by making a simple HTTP request through the proxy
    match test_proxy_connectivity_for_provider(&request).await {
        Ok(()) => Ok(Json(TestProviderProxyResponse {
            success: true,
            message: "Proxy connection successful".to_string(),
        })),
        Err(e) => Ok(Json(TestProviderProxyResponse {
            success: false,
            message: format!("Proxy connection failed: {}", e),
        })),
    }
}

async fn test_proxy_connectivity_for_provider(
    proxy_config: &ProviderProxySettings,
) -> ApiResult<()> {
    // Validate proxy URL format
    if proxy_config.url.trim().is_empty() {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Proxy URL is empty",
        ));
    }

    // Parse and validate the proxy URL
    let _proxy_url = reqwest::Url::parse(&proxy_config.url).map_err(|e| {
        AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            format!("Invalid proxy URL format: {}", e),
        )
    })?;

    // Create a reqwest client with proxy configuration
    let mut proxy_builder = reqwest::Proxy::all(&proxy_config.url).map_err(|e| {
        AppError::new(
            crate::api::errors::ErrorCode::SystemInternalError,
            format!("Failed to create proxy: {}", e),
        )
    })?;

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

    // Handle other SSL settings
    if proxy_config.proxy_ssl {
        // Additional proxy SSL configuration if needed
    }

    let client = client_builder.build().map_err(|e| {
        AppError::new(
            crate::api::errors::ErrorCode::SystemInternalError,
            format!("Failed to create HTTP client: {}", e),
        )
    })?;

    // Test the proxy by making a request to a reliable endpoint
    // Using httpbin.org as it's a simple testing service that returns IP info
    let test_url = if proxy_config.enabled {
        "https://httpbin.org/ip"
    } else {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Proxy is not enabled",
        ));
    };

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
                            Err(AppError::new(
                                crate::api::errors::ErrorCode::SystemExternalServiceError,
                                format!("Unexpected response format: {}", body),
                            ))
                        }
                    }
                    Err(e) => Err(AppError::new(
                        crate::api::errors::ErrorCode::SystemExternalServiceError,
                        format!("Failed to read response body: {}", e),
                    )),
                }
            } else {
                Err(AppError::new(
                    crate::api::errors::ErrorCode::SystemExternalServiceError,
                    format!("HTTP request failed with status: {}", response.status()),
                ))
            }
        }
        Err(e) => {
            // Check if it's a proxy-related error
            let error_msg = e.to_string();
            if error_msg.contains("proxy") || error_msg.contains("CONNECT") {
                Err(AppError::new(
                    crate::api::errors::ErrorCode::SystemExternalServiceError,
                    format!("Proxy connection failed: {}", e),
                ))
            } else if error_msg.contains("timeout") {
                Err(AppError::new(
                    crate::api::errors::ErrorCode::SystemExternalServiceError,
                    "Proxy connection timed out",
                ))
            } else if error_msg.contains("dns") {
                Err(AppError::new(
                    crate::api::errors::ErrorCode::SystemExternalServiceError,
                    format!("DNS resolution failed (check proxy settings): {}", e),
                ))
            } else {
                Err(AppError::new(
                    crate::api::errors::ErrorCode::SystemExternalServiceError,
                    format!("Network request failed: {}", e),
                ))
            }
        }
    }
}

// Get groups that have access to a model provider
pub async fn get_provider_groups(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<Vec<UserGroup>>> {
    match user_group_providers::get_groups_for_provider(provider_id).await {
        Ok(groups) => Ok(Json(groups)),
        Err(e) => {
            eprintln!("Error getting groups for model provider: {}", e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

// Start a Candle model
pub async fn start_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Get the model from database
    let model = match providers::get_model_db_by_id(model_id).await {
        Ok(Some(model)) => model,
        Ok(None) => return Err(AppError::not_found("Model")),
        Err(e) => {
            eprintln!("Failed to get model {}: {}", model_id, e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    // Get the provider to check if it's a Candle provider
    let provider = match providers::get_provider_by_id(model.provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => return Err(AppError::not_found("Model provider")),
        Err(e) => {
            eprintln!("Failed to get model provider: {}", e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    if provider.provider_type != "candle" {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Only Candle models can be started",
        ));
    }

    // Check if model is actually running
    let model_manager = crate::ai::candle_server::management::get_model_manager();
    let model_path = model.get_model_path();

    if model_manager.is_model_running(&model_path).await {
        // Model is already running, update its active status in database
        println!(
            "Model {} is already running, updating active status in database",
            model_id
        );

        match providers::update_model(
            model_id,
            UpdateModelRequest {
                name: None,
                alias: None,
                description: None,
                parameters: None,
                enabled: None,
                is_active: Some(true),
                capabilities: None,
                settings: None,
            },
        )
        .await
        {
            Ok(_) => {
                println!("Successfully updated model {} active status", model_id);
                return Ok(StatusCode::OK);
            }
            Err(e) => {
                eprintln!("Failed to update model {} active status: {}", model_id, e);
                return Err(AppError::internal_error("Failed to update model status"));
            }
        }
    }

    // Convert ModelDb to Model to access settings
    let model_with_settings = crate::database::models::Model::from_db(model.clone(), None);

    // Validate that the model files exist
    let model_path = model_with_settings.get_model_path(&model.provider_id);
    if !crate::ai::candle_server::models::ModelUtils::model_exists(&model_path) {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Model files not found or invalid",
        ));
    }

    // Start the model server process

    // Get device configuration from model settings
    let settings = model_with_settings.get_settings();
    let device_ids = settings.device_ids.filter(|ids| !ids.is_empty());

    // Convert device_type from string to DeviceType enum
    let device_type = match settings.device_type.as_deref() {
        Some("cpu") => crate::ai::candle_server::DeviceType::Cpu,
        Some("cuda") => crate::ai::candle_server::DeviceType::Cuda,
        Some("metal") => crate::ai::candle_server::DeviceType::Metal,
        _ => crate::ai::candle_server::DeviceType::Cpu, // Default to CPU if not specified or unknown
    };

    match model_manager
        .start_model(
            &model_id.to_string(),
            model_path,
            settings
                .architecture
                .clone()
                .unwrap_or_else(|| "llama".to_string()), // Use model architecture or default to llama
            device_type, // Use actual device type from model
            device_ids,  // Pass device_ids from model
            // Additional parameters (using defaults for now)
            None, // verbose
            None, // max_num_seqs (default: 256)
            None, // block_size (default: 32)
            None, // weight_file
            None, // dtype
            None, // kvcache_mem_gpu (default: 4096)
            None, // kvcache_mem_cpu (default: 128)
            None, // record_conversation
            None, // holding_time (default: 500)
            None, // multi_process
            None, // log
        )
        .await
    {
        Ok(crate::ai::candle_server::management::ModelStartResult::Started(port)) => {
            println!("Model {} started successfully on port {}", model_id, port);

            // Update model status and port in database
            let pool = crate::database::get_database_pool().map_err(|e| {
                eprintln!("Failed to get database pool: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

            // Update both active status and port
            let update_status_result = providers::update_model(
                model_id,
                UpdateModelRequest {
                    name: None,
                    alias: None,
                    description: None,
                    parameters: None,
                    enabled: None,
                    is_active: Some(true),
                    capabilities: None,
                    settings: None,
                },
            )
            .await;

            let update_port_result =
                crate::database::model_operations::ModelOperations::update_model_port(
                    pool.as_ref(),
                    &model_id,
                    Some(port as i32),
                )
                .await;

            match (update_status_result, update_port_result) {
                (Ok(Some(_)), Ok(_)) => {
                    println!("Successfully updated model {} status and port", model_id);
                    Ok(StatusCode::OK)
                }
                (Ok(None), _) => {
                    // Model started but not found in database, try to stop the model
                    let _ = model_manager.stop_model(&model.get_model_path()).await;
                    Err(AppError::not_found("Model"))
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("Failed to update model {} status/port: {}", model_id, e);
                    // Model started but database update failed, try to stop the model
                    let _ = model_manager.stop_model(&model.get_model_path()).await;
                    Err(AppError::internal_error("Database operation failed"))
                }
            }
        }
        Ok(crate::ai::candle_server::management::ModelStartResult::AlreadyRunning(port)) => {
            println!(
                "Model {} is already running on port {}, updating database status",
                model_id, port
            );

            // Update model status and port in database to ensure they're correct
            let pool = crate::database::get_database_pool().map_err(|e| {
                eprintln!("Failed to get database pool: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

            let update_status_result = providers::update_model(
                model_id,
                UpdateModelRequest {
                    name: None,
                    alias: None,
                    description: None,
                    parameters: None,
                    enabled: None,
                    is_active: Some(true),
                    capabilities: None,
                    settings: None,
                },
            )
            .await;

            let update_port_result =
                crate::database::model_operations::ModelOperations::update_model_port(
                    pool.as_ref(),
                    &model_id,
                    Some(port as i32),
                )
                .await;

            match (update_status_result, update_port_result) {
                (Ok(Some(_)), Ok(_)) => {
                    println!("Successfully updated model {} status and port", model_id);
                    Ok(StatusCode::OK)
                }
                (Ok(None), _) => {
                    eprintln!("Model {} not found in database", model_id);
                    Err(AppError::not_found("Model"))
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("Failed to update model {} status/port: {}", model_id, e);
                    Err(AppError::internal_error("Database operation failed"))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to start model {}: {}", model_id, e);
            Err(AppError::new(
                crate::api::errors::ErrorCode::SystemInternalError,
                format!("Failed to start model: {}", e),
            ))
        }
    }
}

// Stop a Candle model
pub async fn stop_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Get the model from database
    let model = match providers::get_model_db_by_id(model_id).await {
        Ok(Some(model)) => model,
        Ok(None) => return Err(AppError::not_found("Model")),
        Err(e) => {
            eprintln!("Failed to get model {}: {}", model_id, e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    // Get the provider to check if it's a Candle provider
    let provider = match providers::get_provider_by_id(model.provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => return Err(AppError::not_found("Model provider")),
        Err(e) => {
            eprintln!("Failed to get model provider: {}", e);
            return Err(AppError::internal_error("Database operation failed"));
        }
    };

    if provider.provider_type != "candle" {
        return Err(AppError::new(
            crate::api::errors::ErrorCode::ValidInvalidInput,
            "Only Candle models can be stopped",
        ));
    }

    // Check if model is running
    let model_manager = crate::ai::candle_server::management::get_model_manager();
    if !model_manager
        .is_model_running(&model.get_model_path())
        .await
    {
        // Model is not running, but we should still update the database to ensure consistency
        println!(
            "Model {} is not running, updating database status and clearing port",
            model_id
        );

        let pool = crate::database::get_database_pool().map_err(|e| {
            eprintln!("Failed to get database pool: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

        let update_status_result = providers::update_model(
            model_id,
            UpdateModelRequest {
                name: None,
                alias: None,
                description: None,
                parameters: None,
                enabled: None,
                is_active: Some(false),
                capabilities: None,
                settings: None,
            },
        )
        .await;

        let clear_port_result =
            crate::database::model_operations::ModelOperations::update_model_port(
                pool.as_ref(),
                &model_id,
                None, // Clear the port
            )
            .await;

        return match (update_status_result, clear_port_result) {
            (Ok(Some(_)), Ok(_)) => {
                println!(
                    "Successfully updated model {} status and cleared port",
                    model_id
                );
                Ok(StatusCode::OK)
            }
            (Ok(None), _) => Err(AppError::not_found("Model")),
            (Err(e), _) | (_, Err(e)) => {
                eprintln!("Failed to update model {} status/port: {}", model_id, e);
                Err(AppError::internal_error("Database operation failed"))
            }
        };
    }

    // Stop the model server process
    match model_manager.stop_model(&model.get_model_path()).await {
        Ok(()) => {
            println!("Model {} stopped successfully", model_id);

            // Update model status and clear port in database
            let pool = crate::database::get_database_pool().map_err(|e| {
                eprintln!("Failed to get database pool: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

            let update_status_result = providers::update_model(
                model_id,
                UpdateModelRequest {
                    name: None,
                    alias: None,
                    description: None,
                    parameters: None,
                    enabled: None,
                    is_active: Some(false),
                    capabilities: None,
                    settings: None,
                },
            )
            .await;

            let clear_port_result =
                crate::database::model_operations::ModelOperations::update_model_port(
                    pool.as_ref(),
                    &model_id,
                    None, // Clear the port
                )
                .await;

            match (update_status_result, clear_port_result) {
                (Ok(Some(_)), Ok(_)) => {
                    println!(
                        "Successfully updated model {} status and cleared port",
                        model_id
                    );
                    Ok(StatusCode::OK)
                }
                (Ok(None), _) => Err(AppError::not_found("Model")),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("Failed to update model {} status/port: {}", model_id, e);
                    Err(AppError::internal_error("Database operation failed"))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to stop model {}: {}", model_id, e);
            Err(AppError::new(
                crate::api::errors::ErrorCode::SystemInternalError,
                format!("Failed to stop model: {}", e),
            ))
        }
    }
}

// Enable a model
pub async fn enable_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    match providers::update_model(
        model_id,
        UpdateModelRequest {
            name: None,
            alias: None,
            description: None,
            parameters: None,
            enabled: Some(true),
            is_active: None,
            capabilities: None,
            settings: None,
        },
    )
    .await
    {
        Ok(Some(_)) => Ok(StatusCode::OK),
        Ok(None) => Err(AppError::not_found("Model")),
        Err(e) => {
            eprintln!("Failed to enable model {}: {}", model_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

// Disable a model
pub async fn disable_model(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    match providers::update_model(
        model_id,
        UpdateModelRequest {
            name: None,
            alias: None,
            description: None,
            parameters: None,
            enabled: Some(false),
            is_active: None,
            capabilities: None,
            settings: None,
        },
    )
    .await
    {
        Ok(Some(_)) => Ok(StatusCode::OK),
        Ok(None) => Err(AppError::not_found("Model")),
        Err(e) => {
            eprintln!("Failed to disable model {}: {}", model_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

/// List models for a specific provider
pub async fn list_provider_models(
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Model>>> {
    // First verify the user has access to this provider
    let user_providers = match user_group_providers::get_providers_for_user(auth_user.user.id).await
    {
        Ok(providers) => providers,
        Err(e) => {
            eprintln!(
                "Failed to get model providers for user {}: {}",
                auth_user.user.id, e
            );
            return Err(e.into());
        }
    };

    // Check if user has access to this provider
    if !user_providers.iter().any(|p| p.id == provider_id) {
        return Err(AppError::new(
            ErrorCode::AuthzInsufficientPermissions,
            "Access denied to this model provider",
        ));
    }

    // Get models for the provider
    match providers::get_models_for_provider(provider_id).await {
        Ok(models) => Ok(Json(models)),
        Err(e) => {
            eprintln!("Failed to get models for provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed"))
        }
    }
}

/// Get available compute devices for model deployment
pub async fn get_available_devices(
    Extension(_auth_user): Extension<AuthenticatedUser>,
) -> ApiResult<Json<AvailableDevicesResponse>> {
    let devices_response = crate::ai::core::device_detection::detect_available_devices();
    Ok(Json(devices_response))
}
