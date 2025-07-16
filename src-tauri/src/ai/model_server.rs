use crate::ai::candle::{CandleError, CandleModel};
use crate::ai::candle_models::ModelFactory;
#[cfg(feature = "metal")]
use candle_core::backend::BackendDevice;
use crate::ai::openai_types::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use candle_core::{Device, Tensor};
use futures::stream::{Stream};
use serde_json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TokenizerConfig {
    pub eos_token: String,
    pub eos_token_id: u32,
    pub bos_token: String,
    pub bos_token_id: u32,
    pub unk_token: String,
    pub unk_token_id: u32,
    pub chat_template: Option<String>,
    pub model_max_length: u32,
    pub pad_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub max_position_embeddings: u32,
    pub vocab_size: u32,
    pub hidden_size: u32,
    pub model_type: String,
    pub architectures: Vec<String>,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            eos_token: "</s>".to_string(),
            eos_token_id: 2,
            bos_token: "<s>".to_string(),
            bos_token_id: 1,
            unk_token: "<unk>".to_string(),
            unk_token_id: 0,
            chat_template: None,
            model_max_length: 2048,
            pad_token: None,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            max_position_embeddings: 2048,
            vocab_size: 32000,
            hidden_size: 2048,
            model_type: "llama".to_string(),
            architectures: vec!["LlamaForCausalLM".to_string()],
        }
    }
}

pub struct ModelServerState {
    pub model: Arc<Mutex<Box<dyn CandleModel + Send + Sync>>>,
    pub tokenizer: Arc<tokenizers::Tokenizer>,
    pub model_id: String,
    pub model_name: String,
    pub architecture: String,
    pub started_at: i64,
    pub tokenizer_config: TokenizerConfig,
    pub model_config: ModelConfig,
    pub device: Device,
}

impl ModelServerState {
    /// Parse device configuration and create the appropriate device
    fn create_device(device_type: Option<&str>, device_ids: Option<&str>) -> Result<Device, CandleError> {
        match device_type {
            Some("cuda") => {
                // Parse device IDs for CUDA
                if let Some(ids) = device_ids {
                    let device_id_list: Vec<&str> = ids.split(',').map(|s| s.trim()).collect();
                    if let Some(first_id) = device_id_list.first() {
                        // For now, use the first device ID and try to parse as index
                        // If it's a UUID format (starts with "GPU-"), use device 0
                        let device_index = if first_id.starts_with("GPU-") {
                            0 // Default to first CUDA device for UUID format
                        } else {
                            first_id.parse::<usize>().unwrap_or(0)
                        };
                        
                        #[cfg(feature = "cuda")]
                        {
                            Device::cuda_if_available(device_index)
                                .map_err(|e| CandleError::DeviceError(format!("CUDA device {}: {}", device_index, e)))
                        }
                        #[cfg(not(feature = "cuda"))]
                        {
                            println!("Warning: CUDA requested but not available, falling back to CPU");
                            Ok(Device::Cpu)
                        }
                    } else {
                        #[cfg(feature = "cuda")]
                        {
                            Device::cuda_if_available(0)
                                .map_err(|e| CandleError::DeviceError(format!("CUDA device 0: {}", e)))
                        }
                        #[cfg(not(feature = "cuda"))]
                        {
                            println!("Warning: CUDA requested but not available, falling back to CPU");
                            Ok(Device::Cpu)
                        }
                    }
                } else {
                    // No specific device ID, use default CUDA device
                    #[cfg(feature = "cuda")]
                    {
                        Device::cuda_if_available(0)
                            .map_err(|e| CandleError::DeviceError(format!("CUDA device 0: {}", e)))
                    }
                    #[cfg(not(feature = "cuda"))]
                    {
                        println!("Warning: CUDA requested but not available, falling back to CPU");
                        Ok(Device::Cpu)
                    }
                }
            }
            Some("metal") => {
                #[cfg(feature = "metal")]
                {
                    match candle_core::MetalDevice::new(0) {
                        Ok(metal_device) => Ok(Device::Metal(metal_device)),
                        Err(e) => {
                            println!("Warning: Metal device creation failed ({}), falling back to CPU", e);
                            Ok(Device::Cpu)
                        }
                    }
                }
                #[cfg(not(feature = "metal"))]
                {
                    println!("Warning: Metal requested but not available, falling back to CPU");
                    Ok(Device::Cpu)
                }
            }
            Some("cpu") | None => Ok(Device::Cpu),
            Some(unknown) => {
                println!("Warning: Unknown device type '{}', falling back to CPU", unknown);
                Ok(Device::Cpu)
            }
        }
    }
    pub async fn new(
        model_path: &str,
        architecture: &str,
        model_id: &str,
        model_name: &str,
    ) -> Result<Self, CandleError> {
        Self::new_with_device_config(model_path, architecture, model_id, model_name, None, None).await
    }

    pub async fn new_with_device_config(
        model_path: &str,
        architecture: &str,
        model_id: &str,
        model_name: &str,
        device_type: Option<&str>,
        device_ids: Option<&str>,
    ) -> Result<Self, CandleError> {
        println!("Loading model from: {}", model_path);

        let device = Self::create_device(device_type, device_ids)?;
        let model = ModelFactory::create_model(architecture, model_path, &device)?;
        let tokenizer = ModelFactory::load_tokenizer(architecture, model_path)?;

        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Load tokenizer and model configuration from config files
        let tokenizer_config = Self::load_tokenizer_config(model_path);
        let model_config = Self::load_model_config(model_path);

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(tokenizer),
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            architecture: architecture.to_string(),
            started_at,
            tokenizer_config,
            model_config,
            device,
        })
    }

    pub async fn new_with_specific_files(
        model_path: &str,
        architecture: &str,
        model_id: &str,
        model_name: &str,
        config_file: Option<&str>,
        tokenizer_file: Option<&str>,
        weight_file: Option<&str>,
        additional_weight_files: Option<&str>,
        _vocab_file: Option<&str>,
        _special_tokens_file: Option<&str>,
    ) -> Result<Self, CandleError> {
        Self::new_with_specific_files_and_device(
            model_path,
            architecture,
            model_id,
            model_name,
            config_file,
            tokenizer_file,
            weight_file,
            additional_weight_files,
            _vocab_file,
            _special_tokens_file,
            None,
            None,
        ).await
    }

    pub async fn new_with_specific_files_and_device(
        model_path: &str,
        architecture: &str,
        model_id: &str,
        model_name: &str,
        config_file: Option<&str>,
        tokenizer_file: Option<&str>,
        weight_file: Option<&str>,
        additional_weight_files: Option<&str>,
        _vocab_file: Option<&str>,
        _special_tokens_file: Option<&str>,
        device_type: Option<&str>,
        device_ids: Option<&str>,
    ) -> Result<Self, CandleError> {
        println!("Loading model from: {} with specific files", model_path);
        
        if let Some(config) = config_file {
            println!("  Config file: {}", config);
        }
        if let Some(tokenizer) = tokenizer_file {
            println!("  Tokenizer file: {}", tokenizer);
        }
        if let Some(weight) = weight_file {
            println!("  Weight file: {}", weight);
        }
        if let Some(additional) = additional_weight_files {
            println!("  Additional weight files: {}", additional);
        }

        let device = Self::create_device(device_type, device_ids)?;
        
        // For now, use the existing factory methods but with specific file awareness
        // TODO: Update ModelFactory to accept specific file paths
        let model = ModelFactory::create_model_with_files(
            architecture, 
            model_path, 
            &device,
            config_file,
            weight_file,
            additional_weight_files
        )?;
        
        let tokenizer = ModelFactory::load_tokenizer_with_file(
            architecture, 
            model_path,
            tokenizer_file
        )?;

        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Load tokenizer and model configuration from config files
        let tokenizer_config = Self::load_tokenizer_config(model_path);
        let model_config = Self::load_model_config(model_path);

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(tokenizer),
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            architecture: architecture.to_string(),
            started_at,
            tokenizer_config,
            model_config,
            device,
        })
    }

    fn load_tokenizer_config(model_path: &str) -> TokenizerConfig {
        use std::path::Path;
        
        let tokenizer_config_path = Path::new(model_path).join("tokenizer_config.json");
        let mut config = TokenizerConfig::default();
        
        if let Ok(content) = std::fs::read_to_string(&tokenizer_config_path) {
            if let Ok(json_config) = serde_json::from_str::<serde_json::Value>(&content) {
                // Load chat template
                if let Some(template) = json_config.get("chat_template") {
                    if let Some(template_str) = template.as_str() {
                        config.chat_template = Some(template_str.to_string());
                    }
                }
                
                // Load special tokens
                if let Some(eos_token) = json_config.get("eos_token") {
                    if let Some(eos_str) = eos_token.as_str() {
                        config.eos_token = eos_str.to_string();
                    }
                }
                
                if let Some(bos_token) = json_config.get("bos_token") {
                    if let Some(bos_str) = bos_token.as_str() {
                        config.bos_token = bos_str.to_string();
                    }
                }
                
                if let Some(unk_token) = json_config.get("unk_token") {
                    if let Some(unk_str) = unk_token.as_str() {
                        config.unk_token = unk_str.to_string();
                    }
                }
                
                // Load model max length
                if let Some(max_len) = json_config.get("model_max_length") {
                    if let Some(max_len_num) = max_len.as_u64() {
                        config.model_max_length = max_len_num as u32;
                    }
                }
                
                // Load pad token
                if let Some(pad_token) = json_config.get("pad_token") {
                    if let Some(pad_str) = pad_token.as_str() {
                        config.pad_token = Some(pad_str.to_string());
                    }
                }
                
                // Load token IDs from added_tokens_decoder
                if let Some(added_tokens) = json_config.get("added_tokens_decoder") {
                    if let Some(added_tokens_obj) = added_tokens.as_object() {
                        for (token_id_str, token_info) in added_tokens_obj {
                            if let Ok(token_id) = token_id_str.parse::<u32>() {
                                if let Some(token_content) = token_info.get("content") {
                                    if let Some(content_str) = token_content.as_str() {
                                        match content_str {
                                            "</s>" => config.eos_token_id = token_id,
                                            "<s>" => config.bos_token_id = token_id,
                                            "<unk>" => config.unk_token_id = token_id,
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("Loaded tokenizer config: EOS='{}' (ID: {}), BOS='{}' (ID: {}), UNK='{}' (ID: {})", 
                         config.eos_token, config.eos_token_id,
                         config.bos_token, config.bos_token_id,
                         config.unk_token, config.unk_token_id);
            }
        }
        
        config
    }

    fn load_model_config(model_path: &str) -> ModelConfig {
        use std::path::Path;
        
        let model_config_path = Path::new(model_path).join("config.json");
        let mut config = ModelConfig::default();
        
        if let Ok(content) = std::fs::read_to_string(&model_config_path) {
            if let Ok(json_config) = serde_json::from_str::<serde_json::Value>(&content) {
                // Load max position embeddings
                if let Some(max_pos) = json_config.get("max_position_embeddings") {
                    if let Some(max_pos_num) = max_pos.as_u64() {
                        config.max_position_embeddings = max_pos_num as u32;
                    }
                }
                
                // Load vocab size
                if let Some(vocab_size) = json_config.get("vocab_size") {
                    if let Some(vocab_num) = vocab_size.as_u64() {
                        config.vocab_size = vocab_num as u32;
                    }
                }
                
                // Load hidden size
                if let Some(hidden_size) = json_config.get("hidden_size") {
                    if let Some(hidden_num) = hidden_size.as_u64() {
                        config.hidden_size = hidden_num as u32;
                    }
                }
                
                // Load model type
                if let Some(model_type) = json_config.get("model_type") {
                    if let Some(model_type_str) = model_type.as_str() {
                        config.model_type = model_type_str.to_string();
                    }
                }
                
                // Load architectures
                if let Some(architectures) = json_config.get("architectures") {
                    if let Some(arch_array) = architectures.as_array() {
                        let mut arch_vec = Vec::new();
                        for arch in arch_array {
                            if let Some(arch_str) = arch.as_str() {
                                arch_vec.push(arch_str.to_string());
                            }
                        }
                        if !arch_vec.is_empty() {
                            config.architectures = arch_vec;
                        }
                    }
                }
                
                println!("Loaded model config: max_pos_embeddings={}, vocab_size={}, hidden_size={}, model_type={}", 
                         config.max_position_embeddings, config.vocab_size, config.hidden_size, config.model_type);
            }
        }
        
        config
    }

    fn apply_chat_template(&self, messages: &[ChatMessage]) -> String {
        if let Some(template) = &self.tokenizer_config.chat_template {
            self.render_chat_template(template, messages)
        } else {
            // Fallback to default template
            self.default_chat_template(messages)
        }
    }

    fn render_chat_template(&self, template: &str, messages: &[ChatMessage]) -> String {
        // Basic Jinja2-like template renderer
        // This is a simplified implementation that handles the common chat template pattern
        
        let eos_token = &self.tokenizer_config.eos_token;
        let mut result = String::new();
        
        // Check if this looks like the expected template format
        if template.contains("{% for message in messages %}") && template.contains("message['role']") {
            // Parse the template for role-specific patterns
            let user_template = if template.contains("'user'") {
                // Extract pattern between 'user' condition and next elif/endif
                self.extract_role_template(template, "user", eos_token)
            } else {
                format!("<|user|>\n{{}}{}", eos_token) // fallback
            };
            
            let system_template = if template.contains("'system'") {
                self.extract_role_template(template, "system", eos_token)
            } else {
                format!("<|system|>\n{{}}{}", eos_token) // fallback
            };
            
            let assistant_template = if template.contains("'assistant'") {
                self.extract_role_template(template, "assistant", eos_token)
            } else {
                format!("<|assistant|>\n{{}}{}", eos_token) // fallback
            };
            
            // Apply the template to each message
            for message in messages {
                let template_str = match message.role.as_str() {
                    "user" => &user_template,
                    "system" => &system_template,
                    "assistant" => &assistant_template,
                    _ => &format!("<|{}|>\n{{}}{}", message.role, eos_token),
                };
                
                result.push_str(&template_str.replace("{}", &message.content));
            }
            
            // Add generation prompt if template indicates it
            if template.contains("add_generation_prompt") && template.contains("<|assistant|>") {
                result.push_str("<|assistant|>");
            }
        } else {
            // Fallback to simple format if template is unrecognized
            result = self.default_chat_template(messages);
        }
        
        result
    }
    
    fn extract_role_template(&self, template: &str, role: &str, eos_token: &str) -> String {
        // Simple pattern extraction for role-specific templates
        // Look for patterns like {{ '<|user|>\n' + message['content'] + eos_token }}
        
        let role_pattern = format!("'{}' %}}{{{{ '", role);
        if let Some(start_pos) = template.find(&role_pattern) {
            let start = start_pos + role_pattern.len();
            if let Some(template_part) = template.get(start..) {
                if let Some(end_pos) = template_part.find("' + message['content'] + eos_token }}") {
                    let role_prefix = &template_part[..end_pos];
                    return format!("{}{{}}{}", role_prefix, eos_token);
                }
            }
        }
        
        // Fallback pattern
        format!("<|{}|>\n{{}}{}", role, eos_token)
    }

    fn default_chat_template(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        let eos_token = &self.tokenizer_config.eos_token;

        for message in messages {
            match message.role.as_str() {
                "system" => prompt.push_str(&format!("<|system|>\n{}{}", message.content, eos_token)),
                "user" => prompt.push_str(&format!("<|user|>\n{}{}", message.content, eos_token)),
                "assistant" => prompt.push_str(&format!("<|assistant|>\n{}{}", message.content, eos_token)),
                _ => prompt.push_str(&format!("<|{}|>\n{}{}", message.role, message.content, eos_token)),
            }
        }

        prompt.push_str("<|assistant|>\n");
        prompt
    }
}

pub fn create_model_server_router(state: ModelServerState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(completions))
        .route("/v1/models", get(list_models))
        .route("/v1/models/{model_id}", get(get_model))
        .route("/health", get(health_check))
        .route("/shutdown", post(shutdown_server))
        .with_state(Arc::new(state))
}

/// OpenAI-compatible chat completions endpoint
async fn chat_completions(
    State(state): State<Arc<ModelServerState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    if request.stream {
        stream_chat_completion(state, request).await
    } else {
        non_stream_chat_completion(state, request).await
    }
}

async fn non_stream_chat_completion(
    state: Arc<ModelServerState>,
    request: ChatCompletionRequest,
) -> Response {
    // Convert chat messages to a single prompt using chat template
    let prompt = state.apply_chat_template(&request.messages);
    
    // Print the initial prompt for debugging purposes
    println!("=== INITIAL PROMPT ===");
    println!("{}", prompt);
    println!("=== END INITIAL PROMPT ===");

    // Tokenize input
    let tokens = match state.tokenizer.encode(prompt.clone(), true) {
        Ok(encoding) => encoding.get_ids().to_vec(),
        Err(e) => {
            return Json(ErrorResponse::invalid_request(&format!(
                "Tokenization failed: {}",
                e
            )))
            .into_response();
        }
    };

    // Generate response
    let response_text = match generate_text(&state, &tokens, &request).await {
        Ok(text) => text,
        Err(e) => {
            return Json(ErrorResponse::server_error(&format!(
                "Generation failed: {}",
                e
            )))
            .into_response();
        }
    };

    let response_id = format!("chatcmpl-{}", Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let response = ChatCompletionResponse {
        id: response_id,
        object: "chat.completion".to_string(),
        created,
        model: state.model_name.clone(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: response_text.clone(),
                name: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: Usage {
            prompt_tokens: tokens.len() as i32,
            completion_tokens: estimate_tokens(&response_text),
            total_tokens: tokens.len() as i32 + estimate_tokens(&response_text),
        },
    };

    Json(response).into_response()
}

async fn stream_chat_completion(
    state: Arc<ModelServerState>,
    request: ChatCompletionRequest,
) -> Response {
    let prompt = state.apply_chat_template(&request.messages);
    
    // Print the initial prompt for debugging purposes  
    println!("=== INITIAL PROMPT (STREAMING) ===");
    println!("{}", prompt);
    println!("=== END INITIAL PROMPT (STREAMING) ===");

    let tokens = match state.tokenizer.encode(prompt, true) {
        Ok(encoding) => encoding.get_ids().to_vec(),
        Err(e) => {
            return Json(ErrorResponse::invalid_request(&format!(
                "Tokenization failed: {}",
                e
            )))
            .into_response();
        }
    };

    let response_id = format!("chatcmpl-{}", Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let stream = generate_text_stream(state.clone(), tokens, request, response_id.clone(), created);

    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(30))
                .text("keepalive"),
        )
        .into_response()
}

/// Legacy completions endpoint
async fn completions(
    State(state): State<Arc<ModelServerState>>,
    Json(request): Json<CompletionRequest>,
) -> Response {
    // Print the initial prompt for debugging purposes
    println!("=== INITIAL PROMPT (COMPLETION) ===");
    println!("{}", request.prompt);
    println!("=== END INITIAL PROMPT (COMPLETION) ===");

    let tokens = match state.tokenizer.encode(request.prompt.clone(), true) {
        Ok(encoding) => encoding.get_ids().to_vec(),
        Err(e) => {
            return Json(ErrorResponse::invalid_request(&format!(
                "Tokenization failed: {}",
                e
            )))
            .into_response();
        }
    };

    let chat_request = ChatCompletionRequest {
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: request.prompt.clone(),
            name: None,
        }],
        model: request.model,
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        max_tokens: request.max_tokens,
        stream: request.stream,
        stop: request.stop,
        frequency_penalty: None,
        presence_penalty: None,
        user: None,
    };

    let response_text = match generate_text(&state, &tokens, &chat_request).await {
        Ok(text) => text,
        Err(e) => {
            return Json(ErrorResponse::server_error(&format!(
                "Generation failed: {}",
                e
            )))
            .into_response();
        }
    };

    let response_id = format!("cmpl-{}", Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let response = CompletionResponse {
        id: response_id,
        object: "text_completion".to_string(),
        created,
        model: state.model_name.clone(),
        choices: vec![CompletionChoice {
            text: response_text.clone(),
            index: 0,
            finish_reason: Some("stop".to_string()),
        }],
        usage: Usage {
            prompt_tokens: tokens.len() as i32,
            completion_tokens: estimate_tokens(&response_text),
            total_tokens: tokens.len() as i32 + estimate_tokens(&response_text),
        },
    };

    Json(response).into_response()
}

async fn list_models(State(state): State<Arc<ModelServerState>>) -> Json<ModelsResponse> {
    let model_info = ModelInfo {
        id: state.model_id.clone(),
        object: "model".to_string(),
        created: state.started_at,
        owned_by: "candle".to_string(),
        permission: vec![],
        root: state.model_id.clone(),
        parent: None,
    };

    Json(ModelsResponse {
        object: "list".to_string(),
        data: vec![model_info],
    })
}

async fn get_model(
    State(state): State<Arc<ModelServerState>>,
    Path(model_id): Path<String>,
) -> Response {
    if model_id != state.model_id {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::invalid_request("Model not found")),
        )
            .into_response();
    }

    let model_info = ModelInfo {
        id: state.model_id.clone(),
        object: "model".to_string(),
        created: state.started_at,
        owned_by: "candle".to_string(),
        permission: vec![],
        root: state.model_id.clone(),
        parent: None,
    };

    Json(model_info).into_response()
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}

async fn shutdown_server() -> Json<serde_json::Value> {
    // TODO: Implement graceful shutdown
    Json(serde_json::json!({
        "message": "Shutdown initiated"
    }))
}

// Helper functions


async fn generate_text(
    state: &Arc<ModelServerState>,
    tokens: &[u32],
    request: &ChatCompletionRequest,
) -> Result<String, CandleError> {
    use candle_core::Tensor;
    
    
    let mut model = state.model.lock().await;
    let device = &state.device;
    
    let max_tokens = request.max_tokens.unwrap_or(100).min(state.model_config.max_position_embeddings as i32);
    let temperature = request.temperature.unwrap_or(0.7);
    
    // Clear the cache for fresh generation
    model.clear_cache();
    
    let mut generated_tokens = tokens.to_vec();
    let mut new_tokens = Vec::new(); // Track only newly generated tokens
    let mut generated_text = String::new();
    
    // Process the input tokens first
    let input_ids = Tensor::from_slice(tokens, (1, tokens.len()), device)
        .map_err(|e| CandleError::InferenceError(format!("Failed to create input tensor: {}", e)))?;
    
    let logits = model.forward(&input_ids, 0)
        .map_err(|e| CandleError::InferenceError(format!("Model forward pass failed: {}", e)))?;
    
    // Generate tokens one by one
    for step in 0..max_tokens {
        // For autoregressive generation, we only pass the last token and use start_pos > 0
        let start_pos = tokens.len() + step as usize;
        let last_token = if step == 0 {
            // Get the last token from the logits we just computed
            let last_token_logits = if logits.rank() == 2 {
                // For 2D logits [batch_size, vocab_size], just take the first row
                logits.narrow(0, 0, 1)?.squeeze(0)?
            } else if logits.rank() == 3 {
                // For 3D logits [batch_size, seq_len, vocab_size], take the last position
                logits.narrow(1, logits.dim(1)? - 1, 1)?.squeeze(1)?
            } else {
                return Err(CandleError::InferenceError(format!("Unexpected logits shape: {:?}", logits.shape())));
            };
            
            
            // Apply temperature and sample
            let scaled_logits = if temperature > 0.0 {
                last_token_logits.affine(1.0 / temperature as f64, 0.0)?
            } else {
                last_token_logits
            };
            
            sample_token_improved(&scaled_logits, temperature as f64)?
        } else {
            // For subsequent tokens, run forward pass with just the last token
            let last_token_tensor = Tensor::from_slice(&[generated_tokens[generated_tokens.len() - 1]], (1, 1), device)
                .map_err(|e| CandleError::InferenceError(format!("Failed to create token tensor: {}", e)))?;
            
            let logits = model.forward(&last_token_tensor, start_pos)
                .map_err(|e| CandleError::InferenceError(format!("Model forward pass failed: {}", e)))?;
            
            // Get logits for the last token position
            let last_token_logits = if logits.rank() == 2 {
                // For 2D logits [batch_size, vocab_size], just take the first row
                logits.narrow(0, 0, 1)?.squeeze(0)?
            } else if logits.rank() == 3 {
                // For 3D logits [batch_size, seq_len, vocab_size], take the last position
                logits.narrow(1, logits.dim(1)? - 1, 1)?.squeeze(1)?
            } else {
                return Err(CandleError::InferenceError(format!("Unexpected logits shape: {:?}", logits.shape())));
            };
            
            
            // Apply temperature and sample
            let scaled_logits = if temperature > 0.0 {
                last_token_logits.affine(1.0 / temperature as f64, 0.0)?
            } else {
                last_token_logits
            };
            
            sample_token_improved(&scaled_logits, temperature as f64)?
        };
        
        
        generated_tokens.push(last_token);
        new_tokens.push(last_token);
        
        // Decode the new token and check for stop conditions
        match state.tokenizer.decode(&[last_token], false) {
            Ok(token_text) => {
                generated_text.push_str(&token_text);
                
                // Check for stop sequences
                if let Some(stop_sequences) = &request.stop {
                    for stop_seq in stop_sequences {
                        if generated_text.ends_with(stop_seq) {
                            // Remove the stop sequence from the output
                            generated_text.truncate(generated_text.len() - stop_seq.len());
                            return Ok(generated_text);
                        }
                    }
                }
                
                // Stop if we see common EOS patterns in the text
                if token_text.trim() == "</s>" || token_text.trim() == "<|endoftext|>" {
                    break;
                }
            }
            Err(_) => {
                // Continue generation even if one token fails to decode
            }
        }
    }
    
    // Decode only the newly generated tokens to get proper spacing
    let final_text = if !new_tokens.is_empty() {
        match state.tokenizer.decode(&new_tokens, true) {
            Ok(text) => text,
            Err(_) => generated_text
        }
    } else {
        generated_text
    };
    
    // Return some fallback text if generation produced nothing
    if final_text.trim().is_empty() {
        Ok("I'm here to help!".to_string())
    } else {
        Ok(final_text)
    }
}

// Simple sampling function - takes the most likely token
fn sample_token(logits: &Tensor) -> Result<u32, CandleError> {
    // Handle both 1D and 2D tensors
    let logits_vec = if logits.rank() == 2 {
        // If 2D, flatten to 1D first
        logits.flatten_all()?.to_vec1::<f32>()
            .map_err(|e| CandleError::InferenceError(format!("Failed to convert 2D logits to vec: {}", e)))?
    } else {
        logits.to_vec1::<f32>()
            .map_err(|e| CandleError::InferenceError(format!("Failed to convert logits to vec: {}", e)))?
    };
    
    let max_index = logits_vec.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    
    Ok(max_index as u32)
}

// Improved sampling function with temperature and top-k filtering
fn sample_token_improved(logits: &Tensor, temperature: f64) -> Result<u32, CandleError> {
    // Handle both 1D and 2D tensors
    let mut logits_vec = if logits.rank() == 2 {
        logits.flatten_all()?.to_vec1::<f32>()
            .map_err(|e| CandleError::InferenceError(format!("Failed to convert 2D logits to vec: {}", e)))?
    } else {
        logits.to_vec1::<f32>()
            .map_err(|e| CandleError::InferenceError(format!("Failed to convert logits to vec: {}", e)))?
    };
    
    // Get top tokens for sampling
    let mut indexed_logits: Vec<(usize, f32)> = logits_vec.iter().enumerate().map(|(i, &v)| (i, v)).collect();
    indexed_logits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    // If temperature is very low, just take argmax
    if temperature < 0.01 {
        let max_index = indexed_logits[0].0;
        return Ok(max_index as u32);
    }
    
    // Apply top-k filtering (k=50)
    let top_k = 50.min(logits_vec.len());
    
    // Zero out all but top-k logits
    for i in top_k..logits_vec.len() {
        let idx = indexed_logits[i].0;
        logits_vec[idx] = f32::NEG_INFINITY;
    }
    
    // Apply softmax with temperature
    let max_logit = logits_vec.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let mut exp_logits: Vec<f32> = logits_vec.iter()
        .map(|&x| ((x - max_logit) / temperature as f32).exp())
        .collect();
    
    let sum: f32 = exp_logits.iter().sum();
    if sum > 0.0 {
        for val in &mut exp_logits {
            *val /= sum;
        }
    }
    
    // Sample from the distribution
    let rand_val: f32 = rand::random();
    let mut cumsum = 0.0;
    
    for (i, &prob) in exp_logits.iter().enumerate() {
        cumsum += prob;
        if rand_val <= cumsum {
            return Ok(i as u32);
        }
    }
    
    // Fallback to argmax
    Ok(indexed_logits[0].0 as u32)
}

fn generate_text_stream(
    state: Arc<ModelServerState>,
    tokens: Vec<u32>,
    request: ChatCompletionRequest,
    response_id: String,
    created: i64,
) -> impl Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> {
    use candle_core::Tensor;
    
    // Clone values for async closure
    let final_response_id = response_id.clone();
    let final_model_name = state.model_name.clone();
    
    // Create async stream that generates tokens one by one
    async_stream::stream! {
        let device = &state.device;
        let max_tokens = request.max_tokens.unwrap_or(100).min(state.model_config.max_position_embeddings as i32);
        let temperature = request.temperature.unwrap_or(0.7);
        
        // Lock the model for the entire generation process
        let mut model = state.model.lock().await;
        
        // Clear the cache for fresh generation
        model.clear_cache();
        
        // Convert input tokens to tensor
        let input_ids = match Tensor::from_slice(&tokens, (1, tokens.len()), device) {
            Ok(tensor) => tensor,
            Err(_) => {
                // Send error and return
                let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                return;
            }
        };
        
        let mut generated_tokens = tokens.clone();
        let mut new_tokens = Vec::new(); // Track only newly generated tokens
        let mut current_input = input_ids;
        let mut _token_count = 0;
        let mut previous_text = String::new(); // Track previously sent text for incremental streaming
        
        // Send initial chunk with role
        let initial_chunk = ChatCompletionChunk {
            id: response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: state.model_name.clone(),
            choices: vec![ChatChoiceDelta {
                index: 0,
                delta: ChatMessageDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        
        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&initial_chunk).unwrap()));
        
        // Process initial input tokens
        let mut logits = match model.forward(&current_input, 0) {
            Ok(logits) => logits,
            Err(e) => {
                eprintln!("ERROR: Initial forward pass failed: {:?}", e);
                let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                return;
            }
        };
        
        // Generate tokens one by one
        for step in 0..max_tokens {
            let start_pos = tokens.len() + step as usize;
            let next_token = if step == 0 {
                // Get the last token from the logits we just computed
                let last_token_logits = if logits.rank() == 2 {
                    match logits.narrow(0, 0, 1).and_then(|t| t.squeeze(0)) {
                        Ok(logits) => logits,
                        Err(e) => {
                            eprintln!("ERROR: First token logits processing failed (2D): {:?}", e);
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        }
                    }
                } else if logits.rank() == 3 {
                    match logits.dim(1) {
                        Ok(dim1) if dim1 > 0 => {
                            match logits.narrow(1, dim1 - 1, 1).and_then(|t| t.squeeze(1)) {
                                Ok(logits) => logits,
                                Err(e) => {
                                    eprintln!("ERROR: 3D logits narrow/squeeze failed: {:?}", e);
                                    let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                                    yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                                    return;
                                }
                            }
                        },
                        Ok(dim1) => {
                            eprintln!("ERROR: Invalid logits dimension 1: {}", dim1);
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        },
                        Err(e) => {
                            eprintln!("ERROR: Failed to get logits dimension 1: {:?}", e);
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        }
                    }
                } else {
                    let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                    yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                    return;
                };
                
                // Apply temperature and sample
                let scaled_logits = if temperature > 0.0 {
                    match last_token_logits.affine(1.0 / temperature as f64, 0.0) {
                        Ok(logits) => logits,
                        Err(_) => last_token_logits,
                    }
                } else {
                    last_token_logits
                };
                
                match sample_token_improved(&scaled_logits, temperature as f64) {
                    Ok(token) => token,
                    Err(_) => {
                        let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                        return;
                    }
                }
            } else {
                // For subsequent tokens, run forward pass with just the last token
                let last_token_tensor = match Tensor::from_slice(&[generated_tokens[generated_tokens.len() - 1]], (1, 1), device) {
                    Ok(tensor) => tensor,
                    Err(_) => {
                        let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                        return;
                    }
                };
                
                logits = match model.forward(&last_token_tensor, start_pos) {
                    Ok(logits) => logits,
                    Err(_) => {
                        let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                        return;
                    }
                };
                
                // Get logits for the last token position
                let last_token_logits = if logits.rank() == 2 {
                    match logits.narrow(0, 0, 1).and_then(|t| t.squeeze(0)) {
                        Ok(logits) => logits,
                        Err(_) => {
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        }
                    }
                } else if logits.rank() == 3 {
                    match logits.dim(1) {
                        Ok(dim1) if dim1 > 0 => {
                            match logits.narrow(1, dim1 - 1, 1).and_then(|t| t.squeeze(1)) {
                                Ok(logits) => logits,
                                Err(e) => {
                                    eprintln!("ERROR: Subsequent 3D logits narrow/squeeze failed: {:?}", e);
                                    let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                                    yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                                    return;
                                }
                            }
                        },
                        Ok(dim1) => {
                            eprintln!("ERROR: Invalid subsequent logits dimension 1: {}", dim1);
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        },
                        Err(e) => {
                            eprintln!("ERROR: Failed to get subsequent logits dimension 1: {:?}", e);
                            let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                            yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                            return;
                        }
                    }
                } else {
                    let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                    yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                    return;
                };
                
                // Apply temperature and sample
                let scaled_logits = if temperature > 0.0 {
                    match last_token_logits.affine(1.0 / temperature as f64, 0.0) {
                        Ok(logits) => logits,
                        Err(_) => last_token_logits,
                    }
                } else {
                    last_token_logits
                };
                
                match sample_token_improved(&scaled_logits, temperature as f64) {
                    Ok(token) => token,
                    Err(_) => {
                        let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                        return;
                    }
                }
            };
            
            generated_tokens.push(next_token);
            new_tokens.push(next_token);
            _token_count += 1;
            
            // For proper streaming with spaces, decode all new tokens and get incremental difference
            let new_content = if let Ok(full_text) = state.tokenizer.decode(&new_tokens, true) {
                // Get only the new content by removing what we've already sent
                let incremental_content = if full_text.len() > previous_text.len() {
                    full_text[previous_text.len()..].to_string()
                } else {
                    String::new()
                };
                
                // Update previous text for next iteration
                previous_text = full_text;
                
                incremental_content
            } else {
                String::new()
            };
            
            // Check for stop sequences and EOS tokens
            let mut should_stop = false;
            
            // Check for EOS tokens that should stop generation
            if next_token == state.tokenizer_config.eos_token_id {
                should_stop = true;
            }
            
            // Check max tokens limit
            if new_tokens.len() >= max_tokens as usize {
                should_stop = true;
            }
            let mut final_content = new_content.clone();
            
            if let Some(stop_sequences) = &request.stop {
                for stop_seq in stop_sequences {
                    if final_content.contains(stop_seq) {
                        // Remove the stop sequence from the output
                        if let Some(pos) = final_content.find(stop_seq) {
                            final_content.truncate(pos);
                        }
                        should_stop = true;
                        break;
                    }
                }
            }
            
            // Send the token chunk
            if !final_content.is_empty() {
                let chunk = ChatCompletionChunk {
                    id: response_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: state.model_name.clone(),
                    choices: vec![ChatChoiceDelta {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: Some(final_content.clone()),
                        },
                        finish_reason: None,
                    }],
                };
                
                yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&chunk).unwrap()));
            }
            
            if should_stop {
                break;
            }
            
            // Add small delay to simulate realistic streaming
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            
            // Update input for next iteration
            if let Ok(new_input) = Tensor::from_slice(&generated_tokens, (1, generated_tokens.len()), device) {
                current_input = new_input;
            } else {
                let error_chunk = create_error_chunk(&response_id, created, &state.model_name);
                yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&error_chunk).unwrap()));
                return;
            }
        }
        
        // Send final chunk
        let final_chunk = ChatCompletionChunk {
            id: final_response_id,
            object: "chat.completion.chunk".to_string(),
            created,
            model: final_model_name,
            choices: vec![ChatChoiceDelta {
                index: 0,
                delta: ChatMessageDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        };
        
        yield Ok(axum::response::sse::Event::default().data(serde_json::to_string(&final_chunk).unwrap()));
    }
}

fn create_error_chunk(response_id: &str, created: i64, model_name: &str) -> ChatCompletionChunk {
    ChatCompletionChunk {
        id: response_id.to_string(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model_name.to_string(),
        choices: vec![ChatChoiceDelta {
            index: 0,
            delta: ChatMessageDelta {
                role: None,
                content: Some("Error: Failed to generate response".to_string()),
            },
            finish_reason: Some("error".to_string()),
        }],
    }
}

fn estimate_tokens(text: &str) -> i32 {
    // Rough estimation: 1 token per 4 characters
    (text.len() / 4).max(1) as i32
}
