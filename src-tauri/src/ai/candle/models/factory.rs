use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::llama::{Cache, Config as LlamaConfig, Llama, LlamaEosToks};
use serde::Deserialize;
use serde_json;
use std::path::Path;
use tokenizers::Tokenizer;

use super::super::candle::{CandleError, CandleModel};
use super::super::quantization::{QuantConfig, QuantMethod, GptqConfig, GptqLoader};

/// Check if two devices are the same by comparing their debug representation
/// This is a workaround since Device doesn't implement PartialEq
fn device_matches(device1: &Device, device2: &Device) -> bool {
    format!("{:?}", device1) == format!("{:?}", device2)
}

#[derive(Debug, Deserialize)]
struct ConfigJson {
    vocab_size: usize,
    hidden_size: usize,
    intermediate_size: usize,
    num_hidden_layers: usize,
    num_attention_heads: usize,
    num_key_value_heads: Option<usize>,
    max_position_embeddings: usize,
    rms_norm_eps: f64,
    rope_theta: Option<f32>,
    bos_token_id: Option<u32>,
    eos_token_id: Option<u32>,
    tie_word_embeddings: Option<bool>,
}

impl ConfigJson {
    fn to_candle_config(&self) -> LlamaConfig {
        LlamaConfig {
            vocab_size: self.vocab_size,
            hidden_size: self.hidden_size,
            intermediate_size: self.intermediate_size,
            num_hidden_layers: self.num_hidden_layers,
            num_attention_heads: self.num_attention_heads,
            num_key_value_heads: self.num_key_value_heads.unwrap_or(self.num_attention_heads),
            max_position_embeddings: self.max_position_embeddings,
            rms_norm_eps: self.rms_norm_eps,
            rope_theta: self.rope_theta.unwrap_or(10000.0),
            bos_token_id: Some(self.bos_token_id.unwrap_or(1)),
            eos_token_id: Some(LlamaEosToks::Single(self.eos_token_id.unwrap_or(2))),
            rope_scaling: None,
            tie_word_embeddings: self.tie_word_embeddings.unwrap_or(false),
            use_flash_attn: false,
        }
    }
}

/// Real Llama model implementation using Candle
#[derive(Debug)]
pub struct LlamaModelWrapper {
    model: Llama,
    device: Device,
    cache: Cache,
    config: LlamaConfig,
}

impl LlamaModelWrapper {
    pub fn load(model_path: &str, device: &Device) -> Result<Self, CandleError> {
        println!("Loading real Llama model from: {}", model_path);

        // Load configuration
        let config_path = Path::new(model_path).join("config.json");
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to read config: {}", e)))?;
        let config_json: ConfigJson = serde_json::from_str(&config_str)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to parse config: {}", e)))?;
        let config = config_json.to_candle_config();

        println!(
            "Model config loaded: vocab_size={}, hidden_size={}",
            config.vocab_size, config.hidden_size
        );

        // Load model weights - try with F16 first as it's more common for Llama models
        let weights_path = Path::new(model_path).join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F16, device)?
        };

        // Create model
        let model = Llama::load(vb, &config)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to load model: {}", e)))?;

        // Initialize cache with F16 to match model
        let cache = Cache::new(true, candle_core::DType::F16, &config, device)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to create cache: {}", e)))?;

        println!("Model loaded successfully!");

        Ok(Self {
            model,
            device: device.clone(),
            cache,
            config,
        })
    }

    pub fn load_tokenizer(model_path: &str) -> Result<Tokenizer, CandleError> {
        let tokenizer_path = Path::new(model_path).join("tokenizer.json");
        println!("Loading tokenizer from: {}", tokenizer_path.display());
        Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| CandleError::TokenizerError(format!("Failed to load tokenizer: {}", e)))
    }

    pub fn load_with_files(
        model_path: &str,
        device: &Device,
        config_file: Option<&str>,
        weight_file: Option<&str>,
        _additional_weight_files: Option<&str>,
    ) -> Result<Self, CandleError> {
        println!(
            "Loading Llama model from: {} with specific files",
            model_path
        );

        // Use specific config file if provided
        let config_path = if let Some(config_file) = config_file {
            Path::new(model_path).join(config_file)
        } else {
            Path::new(model_path).join("config.json")
        };

        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to read config: {}", e)))?;
        let config_json: ConfigJson = serde_json::from_str(&config_str)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to parse config: {}", e)))?;
        let config = config_json.to_candle_config();

        // Use specific weight file if provided
        let weights_path = if let Some(weight_file) = weight_file {
            Path::new(model_path).join(weight_file)
        } else {
            Path::new(model_path).join("model.safetensors")
        };

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F16, device)?
        };

        let model = Llama::load(vb, &config)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to load model: {}", e)))?;

        let cache = Cache::new(true, candle_core::DType::F16, &config, device)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to create cache: {}", e)))?;

        Ok(Self {
            model,
            device: device.clone(),
            cache,
            config,
        })
    }

    pub fn load_gguf(
        model_path: &str,
        device: &Device,
        weight_file: Option<&str>,
    ) -> Result<Self, CandleError> {
        // GGUF loading would require different implementation
        // For now, fall back to regular loading
        Self::load(model_path, device)
    }

    pub fn load_tokenizer_with_file(
        model_path: &str,
        tokenizer_file: Option<&str>,
    ) -> Result<Tokenizer, CandleError> {
        let absolute_path = crate::APP_DATA_DIR.join(model_path);

        let tokenizer_path = if let Some(tokenizer_file) = tokenizer_file {
            absolute_path.join(tokenizer_file)
        } else {
            absolute_path.join("tokenizer.json")
        };

        println!("Loading tokenizer from: {}", tokenizer_path.display());

        Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| CandleError::TokenizerError(format!("Failed to load tokenizer: {}", e)))
    }
}

impl CandleModel for LlamaModelWrapper {
    fn forward(&mut self, input_ids: &Tensor, start_pos: usize) -> candle_core::Result<Tensor> {
        // Note: Input tensors should now be created on the correct device directly
        // If this message appears, there's still a device mismatch that should be investigated
        if !device_matches(input_ids.device(), &self.device) {
            println!(
                "WARNING: Input tensor device mismatch - tensor: {:?}, model: {:?}",
                input_ids.device(),
                self.device
            );
        }

        let real_logits = self.model.forward(&input_ids, start_pos, &mut self.cache)?;

        // DEBUG: Check what the real logits look like
        if let Ok(logits_vec) = real_logits.to_vec2::<f32>() {
            if !logits_vec.is_empty() && !logits_vec[0].is_empty() {
                // Get the last token's logits
                let last_token_logits = &logits_vec[0];

                // Find top 10 tokens by probability
                let mut indexed_logits: Vec<(usize, f32)> = last_token_logits
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i, v))
                    .collect();
                indexed_logits
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                // Check if the model is outputting reasonable distributions
                let max_logit = indexed_logits[0].1;
                let _min_logit = indexed_logits.last().map(|(_, v)| *v).unwrap_or(max_logit);

                // Check if UNK token is consistently highest
                if indexed_logits[0].0 == 0 {
                    println!(
                        "WARNING: Model real logits show UNK token (0) as highest: {}",
                        indexed_logits[0].1
                    );

                    // Try to find what tokens should be high
                    let mut non_unk_tokens = Vec::new();
                    for (i, logit) in indexed_logits.iter().take(20) {
                        if *i != 0 && *i != 1 && *i != 2 {
                            // Skip UNK, BOS, EOS
                            non_unk_tokens.push((*i, *logit));
                        }
                    }
                    println!(
                        "Top non-special tokens: {:?}",
                        &non_unk_tokens[..5.min(non_unk_tokens.len())]
                    );
                }
            }
        }

        // Use the actual model logits now
        Ok(real_logits)
    }

    fn forward_with_cache(&self, input_ids: &Tensor, start_pos: usize, cache: &mut Cache) -> candle_core::Result<Tensor> {
        // Check device match
        if !device_matches(input_ids.device(), &self.device) {
            println!(
                "WARNING: Input tensor device mismatch - tensor: {:?}, model: {:?}",
                input_ids.device(),
                self.device
            );
        }

        // Forward pass with external cache
        self.model.forward(input_ids, start_pos, cache)
    }

    fn clear_cache(&mut self) {
        // Reset the cache
        if let Ok(new_cache) = Cache::new(true, candle_core::DType::F16, &self.config, &self.device)
        {
            self.cache = new_cache;
        }
    }

    fn get_config(&self) -> candle_transformers::models::llama::Config {
        self.config.clone()
    }
}

/// Model factory for creating different model types
pub struct ModelFactory;

/// GPTQ-quantized LLaMA model wrapper
#[derive(Debug)]
pub struct GptqLlamaModelWrapper {
    config: LlamaConfig,
    quantization_config: QuantConfig,
    device: Device,
    // For simplicity, we'll fall back to regular model for now
    // In a full implementation, this would contain GPTQ linear layers
    inner_model: LlamaModelWrapper,
}

impl GptqLlamaModelWrapper {
    pub fn load(
        model_path: &str,
        device: &Device,
        quant_config: &QuantConfig,
    ) -> Result<Self, CandleError> {
        // Validate quantization config
        quant_config.validate().map_err(|e| CandleError::ConfigError(e))?;

        println!(
            "Loading GPTQ model with {} bits, group size {}, symmetric: {}",
            quant_config.bits, quant_config.group_size, quant_config.symmetric
        );

        // Load configuration first
        let config_path = Path::new(model_path).join("config.json");
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to read config: {}", e)))?;
        let config_json: ConfigJson = serde_json::from_str(&config_str)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to parse config: {}", e)))?;
        let config = config_json.to_candle_config();

        // Check for GPTQ tensors first to avoid loading wrong format
        let safetensors_path = Path::new(model_path).join("model.safetensors");
        if !safetensors_path.exists() {
            return Err(CandleError::ModelLoadError(
                "GPTQ model.safetensors file not found".to_string()
            ));
        }

        // First check the safetensors file metadata for tensor names without loading
        let safetensors_content = std::fs::read(&safetensors_path)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to read safetensors: {}", e)))?;
        
        // Parse safetensors metadata to check for GPTQ tensor names
        let has_qweight_tensors = {
            // Look for characteristic GPTQ tensor patterns in the file
            let content_str = String::from_utf8_lossy(&safetensors_content);
            content_str.contains(".qweight") && content_str.contains(".scales")
        };

        if has_qweight_tensors {
            println!("✓ Detected GPTQ tensors (qweight, scales, qzeros format)");
            
            // This is a proper GPTQ model - we would implement full GPTQ loading here
            // For now, return an error explaining the limitation
            return Err(CandleError::ModelLoadError(
                "GPTQ tensor loading is detected but not yet fully implemented. \
                The model contains GPTQ tensors (qweight, scales, qzeros) which require \
                specialized loading logic and GPTQ linear layers. \
                This will be implemented in a future update.".to_string()
            ));
        } else {
            println!("⚠️  Warning: Model has quantization_config but uses regular tensor format");
            println!("   This appears to be a regular model misidentified as GPTQ");
            
            // Fall back to regular loading for models that claim to be GPTQ but aren't
            let inner_model = LlamaModelWrapper::load(model_path, device)?;
            
            Ok(Self {
                config: inner_model.get_config(),
                quantization_config: quant_config.clone(),
                device: device.clone(),
                inner_model,
            })
        }
    }
}

impl CandleModel for GptqLlamaModelWrapper {
    fn forward(&mut self, input_ids: &Tensor, start_pos: usize) -> candle_core::Result<Tensor> {
        // For now, delegate to the inner model
        // In a full implementation, this would use GPTQ layers
        self.inner_model.forward(input_ids, start_pos)
    }

    fn forward_with_cache(
        &self,
        input_ids: &Tensor,
        start_pos: usize,
        cache: &mut Cache,
    ) -> candle_core::Result<Tensor> {
        self.inner_model.forward_with_cache(input_ids, start_pos, cache)
    }

    fn clear_cache(&mut self) {
        self.inner_model.clear_cache()
    }

    fn get_config(&self) -> LlamaConfig {
        self.config.clone()
    }
}

impl ModelFactory {
    pub fn create_model(
        model_type: &str,
        model_path: &str,
        device: &Device,
    ) -> Result<Box<dyn CandleModel + Send + Sync>, CandleError> {
        match model_type.to_lowercase().as_str() {
            "llama" => {
                let model = LlamaModelWrapper::load(model_path, device)?;
                Ok(Box::new(model))
            }
            _ => Err(CandleError::UnsupportedModel(model_type.to_string())),
        }
    }

    pub fn create_model_with_files(
        model_type: &str,
        model_path: &str,
        device: &Device,
        config_file: Option<&str>,
        weight_file: Option<&str>,
        additional_weight_files: Option<&str>,
    ) -> Result<Box<dyn CandleModel + Send + Sync>, CandleError> {
        match model_type.to_lowercase().as_str() {
            "llama" => {
                let model = LlamaModelWrapper::load_with_files(
                    model_path,
                    device,
                    config_file,
                    weight_file,
                    additional_weight_files,
                )?;
                Ok(Box::new(model))
            }
            "gguf" => {
                let model = LlamaModelWrapper::load_gguf(model_path, device, weight_file)?;
                Ok(Box::new(model))
            }
            _ => Err(CandleError::UnsupportedModel(model_type.to_string())),
        }
    }

    pub fn load_tokenizer(model_type: &str, model_path: &str) -> Result<Tokenizer, CandleError> {
        match model_type.to_lowercase().as_str() {
            "llama" => LlamaModelWrapper::load_tokenizer(model_path),
            _ => Err(CandleError::UnsupportedModel(model_type.to_string())),
        }
    }

    pub fn load_tokenizer_with_file(
        model_type: &str,
        model_path: &str,
        tokenizer_file: Option<&str>,
    ) -> Result<Tokenizer, CandleError> {
        match model_type.to_lowercase().as_str() {
            "llama" | "gguf" => {
                LlamaModelWrapper::load_tokenizer_with_file(model_path, tokenizer_file)
            }
            _ => Err(CandleError::UnsupportedModel(model_type.to_string())),
        }
    }

    /// Detect quantization format in model directory
    pub fn detect_quantization(model_path: &str) -> Result<QuantConfig, CandleError> {
        let path = Path::new(model_path);
        
        // Try to detect GPTQ first
        if let Ok(Some(gptq_config)) = GptqLoader::detect_gptq_model(path) {
            let quant_config: QuantConfig = gptq_config.into();
            return Ok(quant_config);
        }
        
        // TODO: Add GGUF detection here
        
        // Default to no quantization
        Ok(QuantConfig::default())
    }

    /// Create a quantized model based on detected or specified quantization
    pub fn create_quantized_model(
        model_type: &str,
        model_path: &str,
        device: &Device,
        quant_config: Option<&QuantConfig>,
    ) -> Result<Box<dyn CandleModel + Send + Sync>, CandleError> {
        let quant_config = if let Some(config) = quant_config {
            config.clone()
        } else {
            Self::detect_quantization(model_path)?
        };

        match quant_config.method {
            QuantMethod::None => {
                // Load normal model
                Self::create_model(model_type, model_path, device)
            }
            QuantMethod::Gptq | QuantMethod::Awq | QuantMethod::Marlin => {
                // Load GPTQ quantized model
                Self::create_gptq_model(model_type, model_path, device, &quant_config)
            }
            QuantMethod::Gguf => {
                // Load GGUF model (future implementation)
                Err(CandleError::UnsupportedModel("GGUF not yet implemented".to_string()))
            }
        }
    }

    /// Create a GPTQ quantized model
    pub fn create_gptq_model(
        model_type: &str,
        model_path: &str,
        device: &Device,
        quant_config: &QuantConfig,
    ) -> Result<Box<dyn CandleModel + Send + Sync>, CandleError> {
        match model_type.to_lowercase().as_str() {
            "llama" => {
                // For now, create a wrapper that uses GPTQ linear layers
                let model = GptqLlamaModelWrapper::load(model_path, device, quant_config)?;
                Ok(Box::new(model))
            }
            _ => Err(CandleError::UnsupportedModel(format!(
                "GPTQ not supported for model type: {}",
                model_type
            ))),
        }
    }

    /// Auto-detect model format and quantization
    pub fn auto_detect_and_create(
        model_path: &str,
        device: &Device,
    ) -> Result<Box<dyn CandleModel + Send + Sync>, CandleError> {
        // First detect architecture
        let architecture = Self::detect_model_architecture(model_path)?;
        
        // Then detect quantization
        let quant_config = Self::detect_quantization(model_path)?;
        
        // Create appropriate model
        Self::create_quantized_model(&architecture, model_path, device, Some(&quant_config))
    }

    /// Detect model architecture from config file
    pub fn detect_model_architecture(model_path: &str) -> Result<String, CandleError> {
        let config_path = Path::new(model_path).join("config.json");
        
        if !config_path.exists() {
            return Ok("llama".to_string()); // Default fallback
        }

        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to read config.json: {}", e)))?;
        
        let config: serde_json::Value = serde_json::from_str(&config_content)
            .map_err(|e| CandleError::ModelLoadError(format!("Failed to parse config.json: {}", e)))?;

        // Try model_type first
        if let Some(model_type) = config.get("model_type").and_then(|v| v.as_str()) {
            return Ok(model_type.to_string());
        }

        // Try architectures array
        if let Some(architectures) = config.get("architectures").and_then(|v| v.as_array()) {
            if let Some(arch) = architectures.first().and_then(|v| v.as_str()) {
                let normalized = arch.to_lowercase();
                if normalized.contains("llama") {
                    return Ok("llama".to_string());
                } else if normalized.contains("mistral") {
                    return Ok("mistral".to_string());
                } else if normalized.contains("qwen") {
                    return Ok("qwen".to_string());
                }
                return Ok(normalized);
            }
        }

        // Default fallback
        Ok("llama".to_string())
    }
}

/// Model format specifications for different model types
#[derive(Debug, Clone)]
pub struct ModelFormat {
    pub name: String,
    pub required_files: Vec<String>,       // Required filenames
    pub optional_files: Vec<String>,       // Optional filenames
    pub weight_file_patterns: Vec<String>, // Patterns for weight files (can be multiple)
}

/// Model file paths for a specific model instance
#[derive(Debug, Clone)]
pub struct ModelFilePaths {
    pub config_file: Option<String>,
    pub tokenizer_file: Option<String>,
    pub tokenizer_config_file: Option<String>,
    pub vocab_file: Option<String>,
    pub special_tokens_file: Option<String>,
    pub weight_files: Vec<String>, // Can be multiple weight files
}

/// Utility functions for model management
pub struct ModelUtils;

impl ModelUtils {
    /// Get model format specifications for different architectures
    pub fn get_model_format(architecture: &str) -> ModelFormat {
        match architecture.to_lowercase().as_str() {
            "llama" => ModelFormat {
                name: "Llama".to_string(),
                required_files: vec!["tokenizer.json".to_string(), "config.json".to_string()],
                optional_files: vec![
                    "tokenizer_config.json".to_string(),
                    "special_tokens_map.json".to_string(),
                    "generation_config.json".to_string(),
                ],
                weight_file_patterns: vec![
                    "model.safetensors".to_string(),
                    "pytorch_model.bin".to_string(),
                    "model-*.safetensors".to_string(),
                    "pytorch_model-*.bin".to_string(),
                ],
            },
            "mistral" => ModelFormat {
                name: "Mistral".to_string(),
                required_files: vec!["tokenizer.json".to_string(), "config.json".to_string()],
                optional_files: vec![
                    "tokenizer_config.json".to_string(),
                    "special_tokens_map.json".to_string(),
                    "generation_config.json".to_string(),
                ],
                weight_file_patterns: vec![
                    "model.safetensors".to_string(),
                    "pytorch_model.bin".to_string(),
                    "model-*.safetensors".to_string(),
                    "pytorch_model-*.bin".to_string(),
                ],
            },
            "gguf" => ModelFormat {
                name: "GGUF".to_string(),
                required_files: vec![], // GGUF models are self-contained
                optional_files: vec!["tokenizer.json".to_string(), "config.json".to_string()],
                weight_file_patterns: vec!["*.gguf".to_string()],
            },
            _ => ModelFormat {
                name: "Generic".to_string(),
                required_files: vec!["tokenizer.json".to_string(), "config.json".to_string()],
                optional_files: vec![
                    "tokenizer_config.json".to_string(),
                    "special_tokens_map.json".to_string(),
                ],
                weight_file_patterns: vec![
                    "model.safetensors".to_string(),
                    "pytorch_model.bin".to_string(),
                    "*.safetensors".to_string(),
                    "*.bin".to_string(),
                ],
            },
        }
    }

    /// Detect the model format based on available files in the directory
    pub fn detect_model_format(model_path: &str) -> Result<String, std::io::Error> {
        let absolute_path = crate::APP_DATA_DIR.join(model_path);
        let entries = std::fs::read_dir(&absolute_path)?;

        let mut has_gguf = false;
        let mut has_safetensors = false;
        let mut has_pytorch_bin = false;
        let mut has_config = false;
        let mut has_tokenizer = false;

        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_lowercase();

            if filename.ends_with(".gguf") {
                has_gguf = true;
            } else if filename.ends_with(".safetensors") {
                has_safetensors = true;
            } else if filename.ends_with(".bin") && filename.contains("pytorch") {
                has_pytorch_bin = true;
            } else if filename == "config.json" {
                has_config = true;
            } else if filename == "tokenizer.json" {
                has_tokenizer = true;
            }
        }

        // Determine format based on file patterns
        if has_gguf {
            Ok("gguf".to_string())
        } else if has_config && has_tokenizer {
            if has_safetensors {
                Ok("llama".to_string()) // Default to llama for safetensors
            } else if has_pytorch_bin {
                Ok("llama".to_string()) // Default to llama for pytorch bins
            } else {
                Ok("llama".to_string()) // Default format
            }
        } else {
            Ok("llama".to_string()) // Default fallback
        }
    }

    /// Get specific file paths for a model
    pub fn get_model_file_paths(
        model_path: &str,
        architecture: &str,
    ) -> Result<ModelFilePaths, std::io::Error> {
        let absolute_path = crate::APP_DATA_DIR.join(model_path);
        let format = Self::get_model_format(architecture);

        let mut file_paths = ModelFilePaths {
            config_file: None,
            tokenizer_file: None,
            tokenizer_config_file: None,
            vocab_file: None,
            special_tokens_file: None,
            weight_files: Vec::new(),
        };

        let entries = std::fs::read_dir(&absolute_path)?;
        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_string();
            let file_path = entry.path();

            match filename.as_str() {
                "config.json" => {
                    file_paths.config_file = Some(file_path.to_string_lossy().to_string())
                }
                "tokenizer.json" => {
                    file_paths.tokenizer_file = Some(file_path.to_string_lossy().to_string())
                }
                "tokenizer_config.json" => {
                    file_paths.tokenizer_config_file = Some(file_path.to_string_lossy().to_string())
                }
                "special_tokens_map.json" => {
                    file_paths.special_tokens_file = Some(file_path.to_string_lossy().to_string())
                }
                "vocab.json" | "vocab.txt" => {
                    file_paths.vocab_file = Some(file_path.to_string_lossy().to_string())
                }
                _ => {
                    // Check if this is a weight file
                    for pattern in &format.weight_file_patterns {
                        if Self::matches_pattern(&filename, pattern) {
                            file_paths
                                .weight_files
                                .push(file_path.to_string_lossy().to_string());
                            break;
                        }
                    }
                }
            }
        }

        Ok(file_paths)
    }

    /// Simple pattern matching for filenames
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            let pattern_parts: Vec<&str> = pattern.split('*').collect();
            if pattern_parts.len() == 2 {
                let prefix = pattern_parts[0];
                let suffix = pattern_parts[1];
                filename.starts_with(prefix) && filename.ends_with(suffix)
            } else {
                false
            }
        } else {
            filename == pattern
        }
    }

    /// Check if a model exists at the given path (relative to APP_DATA_DIR)
    pub fn model_exists(model_path: &str) -> bool {
        // Convert relative path to absolute path based on APP_DATA_DIR
        let absolute_path = crate::APP_DATA_DIR.join(model_path);
        println!("Checking model path: {}", absolute_path.display());

        if !absolute_path.exists() || !absolute_path.is_dir() {
            return false;
        }

        // Try to detect the model format and check for required files
        match Self::detect_model_format(model_path) {
            Ok(architecture) => {
                let format = Self::get_model_format(&architecture);

                // Check for required files
                for required_file in &format.required_files {
                    if !absolute_path.join(required_file).exists() {
                        println!("Missing required file: {}", required_file);
                        return false;
                    }
                }

                // Check for at least one weight file
                if let Ok(file_paths) = Self::get_model_file_paths(model_path, &architecture) {
                    if file_paths.weight_files.is_empty() && architecture != "gguf" {
                        println!("No weight files found");
                        return false;
                    }
                }

                true
            }
            Err(e) => {
                println!("Error detecting model format: {}", e);
                false
            }
        }
    }

    /// Get model size in bytes (path relative to APP_DATA_DIR)
    pub fn get_model_size(model_path: &str) -> Result<u64, std::io::Error> {
        // Convert relative path to absolute path based on APP_DATA_DIR
        let absolute_path = crate::APP_DATA_DIR.join(model_path);
        let mut total_size = 0;
        for entry in std::fs::read_dir(&absolute_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                total_size += entry.metadata()?.len();
            }
        }
        Ok(total_size)
    }

    /// List available models in a directory (path relative to APP_DATA_DIR)
    pub fn list_models(models_dir: &str) -> Result<Vec<String>, std::io::Error> {
        // Convert relative path to absolute path based on APP_DATA_DIR
        let absolute_path = crate::APP_DATA_DIR.join(models_dir);
        let mut models = Vec::new();
        for entry in std::fs::read_dir(&absolute_path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Pass relative path to model_exists (models_dir/name)
                    let relative_path = format!("{}/{}", models_dir, name);
                    if Self::model_exists(&relative_path) {
                        models.push(name.to_string());
                    }
                }
            }
        }
        Ok(models)
    }
}
