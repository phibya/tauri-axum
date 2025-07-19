use crate::APP_DATA_DIR;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// Typed settings for individual model performance and batching configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelSettings {
    // Device configuration (kept as requested)
    /// Device type (cpu, cuda, metal, etc.)
    pub device_type: Option<String>,
    /// Array of device IDs for multi-GPU
    pub device_ids: Option<Vec<i32>>,

    // Sequence and memory management
    /// Maximum running sequences at any time (--max-seqs)
    #[serde(default = "default_max_num_seqs")]
    pub max_seqs: usize,
    /// Maximum sequence length (--max-seq-len)
    pub max_seq_len: Option<usize>,
    /// Use no KV cache (--no-kv-cache)
    #[serde(default)]
    pub no_kv_cache: bool,
    /// Truncate sequences that exceed max length (--truncate-sequence)
    #[serde(default)]
    pub truncate_sequence: bool,

    // PagedAttention configuration
    /// GPU memory for KV cache in MBs (--pa-gpu-mem)
    pub paged_attn_gpu_mem: Option<usize>,
    /// GPU memory usage percentage 0-1 (--pa-gpu-mem-usage)
    pub paged_attn_gpu_mem_usage: Option<f32>,
    /// Total context length for KV cache (--pa-ctxt-len)
    pub paged_ctxt_len: Option<usize>,
    /// PagedAttention block size (--pa-blk-size)
    #[serde(default = "default_block_size")]
    pub paged_attn_block_size: usize,
    /// Disable PagedAttention on CUDA (--no-paged-attn)
    #[serde(default)]
    pub no_paged_attn: bool,
    /// Enable PagedAttention on Metal (--paged-attn)
    #[serde(default)]
    pub paged_attn: bool,

    // Performance optimization
    /// Number of prefix caches to hold (--prefix-cache-n)
    #[serde(default = "default_prefix_cache_n")]
    pub prefix_cache_n: usize,
    /// Prompt batching chunk size (--prompt-batchsize)
    pub prompt_chunksize: Option<usize>,

    // Model configuration
    /// Model data type: auto, f16, f32, bf16 (--dtype)
    pub dtype: Option<String>,
    /// In-situ quantization method (--isq)
    pub in_situ_quant: Option<String>,
    /// Model architecture override (--arch for plain models)
    pub architecture: Option<String>,

    // Reproducibility
    /// Seed for reproducible generation (--seed)
    pub seed: Option<u64>,

    // Vision model parameters
    /// Maximum edge length for image resizing (--max-edge)
    pub max_edge: Option<usize>,
    /// Maximum number of images (--max-num-images)
    pub max_num_images: Option<usize>,
    /// Maximum image edge length (--max-image-length)
    pub max_image_length: Option<usize>,

    // Local configuration settings from frontend
    /// Auto unload old models when loading new ones
    #[serde(default)]
    pub auto_unload_old_models: bool,
    /// Enable context shift
    #[serde(default)]
    pub context_shift: bool,
    /// Enable continuous batching
    #[serde(default)]
    pub continuous_batching: bool,
    /// Number of parallel operations (1-16)
    #[serde(default = "default_parallel_operations")]
    pub parallel_operations: usize,
    /// Number of CPU threads (auto if None)
    pub cpu_threads: Option<usize>,
    /// Number of threads for batch processing (same as cpu_threads if None)
    pub threads_batch: Option<usize>,
    /// Enable flash attention
    #[serde(default)]
    pub flash_attention: bool,
    /// Enable caching (inverse of no_kv_cache)
    #[serde(default = "default_caching")]
    pub caching: bool,
    /// KV cache type (q8_0, q4_0, q4_1, q5_0, q5_1)
    pub kv_cache_type: Option<String>,
    /// Enable memory mapping
    #[serde(default)]
    pub mmap: bool,
    /// Hugging Face access token
    pub hugging_face_access_token: Option<String>,
}

// Default value functions for ModelSettings
fn default_max_num_seqs() -> usize {
    16 // Updated to match mistralrs-server default
}

fn default_block_size() -> usize {
    32
}

fn default_prefix_cache_n() -> usize {
    16 // Default prefix cache
}

fn default_parallel_operations() -> usize {
    4 // Default parallel operations
}

fn default_caching() -> bool {
    true // Enable caching by default
}

impl ModelSettings {
    /// Create a new ModelSettings with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create ModelSettings optimized for high throughput
    pub fn high_throughput() -> Self {
        Self {
            device_type: Some("cuda".to_string()),
            device_ids: None,
            max_seqs: 64,
            max_seq_len: Some(8192),
            no_kv_cache: false,
            truncate_sequence: false,
            paged_attn_gpu_mem: Some(8192),
            paged_attn_gpu_mem_usage: Some(0.9),
            paged_ctxt_len: Some(8192),
            paged_attn_block_size: 64,
            no_paged_attn: false,
            paged_attn: true,
            prefix_cache_n: 32,
            prompt_chunksize: Some(1024),
            dtype: Some("f16".to_string()),
            in_situ_quant: None,
            architecture: None,
            seed: None,
            max_edge: None,
            max_num_images: None,
            max_image_length: None,
            auto_unload_old_models: false,
            context_shift: false,
            continuous_batching: true,
            parallel_operations: 8,
            cpu_threads: None,
            threads_batch: None,
            flash_attention: true,
            caching: true,
            kv_cache_type: Some("q4_0".to_string()),
            mmap: true,
            hugging_face_access_token: None,
        }
    }

    /// Create ModelSettings optimized for low latency
    pub fn low_latency() -> Self {
        Self {
            device_type: Some("metal".to_string()),
            device_ids: None,
            max_seqs: 16,
            max_seq_len: Some(2048),
            no_kv_cache: false,
            truncate_sequence: true,
            paged_attn_gpu_mem: Some(2048),
            paged_attn_gpu_mem_usage: Some(0.7),
            paged_ctxt_len: Some(2048),
            paged_attn_block_size: 16,
            no_paged_attn: false,
            paged_attn: true,
            prefix_cache_n: 8,
            prompt_chunksize: Some(512),
            dtype: Some("f16".to_string()),
            in_situ_quant: None,
            architecture: None,
            seed: None,
            max_edge: None,
            max_num_images: None,
            max_image_length: None,
            auto_unload_old_models: true,
            context_shift: true,
            continuous_batching: false,
            parallel_operations: 2,
            cpu_threads: None,
            threads_batch: None,
            flash_attention: false,
            caching: true,
            kv_cache_type: Some("q8_0".to_string()),
            mmap: false,
            hugging_face_access_token: None,
        }
    }

    /// Validate the settings and return errors if any
    pub fn validate(&self) -> Result<(), String> {
        if self.max_seqs == 0 {
            return Err("max_seqs must be greater than 0".to_string());
        }

        if self.paged_attn_block_size == 0 {
            return Err("paged_attn_block_size must be greater than 0".to_string());
        }

        if let Some(gpu_mem) = self.paged_attn_gpu_mem {
            if gpu_mem == 0 {
                return Err("paged_attn_gpu_mem must be greater than 0".to_string());
            }
        }

        if let Some(usage) = self.paged_attn_gpu_mem_usage {
            if usage <= 0.0 || usage > 1.0 {
                return Err("paged_attn_gpu_mem_usage must be between 0 and 1".to_string());
            }
        }

        if self.prefix_cache_n == 0 {
            return Err("prefix_cache_n must be greater than 0".to_string());
        }

        // Reasonable limits
        if self.max_seqs > 2048 {
            return Err("max_seqs should not exceed 2048".to_string());
        }

        if self.paged_attn_block_size > 512 {
            return Err("paged_attn_block_size should not exceed 512".to_string());
        }

        if let Some(gpu_mem) = self.paged_attn_gpu_mem {
            if gpu_mem > 65536 {
                return Err("paged_attn_gpu_mem should not exceed 65536MB (64GB)".to_string());
            }
        }

        if let Some(max_seq_len) = self.max_seq_len {
            if max_seq_len > 131072 {
                return Err("max_seq_len should not exceed 131072 tokens".to_string());
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
    pub alias: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub is_deprecated: bool,
    pub is_active: bool,
    pub capabilities: Option<serde_json::Value>,
    pub parameters: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Additional fields for Candle models (None for other providers)
    pub file_size_bytes: Option<i64>,
    pub validation_status: Option<String>,
    pub validation_issues: Option<Vec<String>>,
    pub port: Option<i32>, // Port number where the model server is running
    pub pid: Option<i32>,  // Process ID of the running model server
    pub settings: Option<ModelSettings>, // Model-specific performance settings
    pub files: Option<Vec<ModelFileInfo>>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Model {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        // Parse capabilities JSON
        let capabilities_json: serde_json::Value = row.try_get("capabilities")?;
        let capabilities = if capabilities_json.is_null() {
            None
        } else {
            Some(capabilities_json)
        };

        // Parse parameters JSON
        let parameters_json: serde_json::Value = row.try_get("parameters")?;
        let parameters = if parameters_json.is_null() {
            None
        } else {
            Some(parameters_json)
        };

        // Parse settings JSON
        let settings_json: serde_json::Value = row.try_get("settings")?;
        let settings = if settings_json.is_null() {
            None
        } else {
            Some(
                serde_json::from_value(settings_json).map_err(|e| sqlx::Error::ColumnDecode {
                    index: "settings".into(),
                    source: Box::new(e),
                })?,
            )
        };

        // Parse validation_issues JSON
        let validation_issues_json: Option<serde_json::Value> = row.try_get("validation_issues")?;
        let validation_issues =
            validation_issues_json.and_then(|v| serde_json::from_value::<Vec<String>>(v).ok());

        Ok(Model {
            id: row.try_get("id")?,
            provider_id: row.try_get("provider_id")?,
            name: row.try_get("name")?,
            alias: row.try_get("alias")?,
            description: row.try_get("description")?,
            enabled: row.try_get("enabled")?,
            is_deprecated: row.try_get("is_deprecated")?,
            is_active: row.try_get("is_active")?,
            capabilities,
            parameters,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            file_size_bytes: row.try_get("file_size_bytes")?,
            validation_status: row.try_get("validation_status")?,
            validation_issues,
            port: row.try_get("port")?,
            pid: row.try_get("pid")?,
            settings,
            files: None, // Files need to be loaded separately
        })
    }
}

// Request/Response structures for models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModelRequest {
    pub provider_id: Uuid,
    pub name: String,
    pub alias: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub capabilities: Option<serde_json::Value>,
    pub settings: Option<ModelSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub alias: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub is_active: Option<bool>,
    pub capabilities: Option<serde_json::Value>,
    pub parameters: Option<serde_json::Value>,
    pub settings: Option<ModelSettings>,
}

// Model file tracking for uploaded files
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModelFile {
    pub id: Uuid,
    pub model_id: Uuid,
    pub filename: String,
    pub file_path: String,
    pub file_size_bytes: i64,
    pub file_type: String,
    pub upload_status: String,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFileInfo {
    pub filename: String,
    pub file_size_bytes: i64,
    pub file_type: String,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ModelUploadResponse {
    pub model_id: Uuid,
    pub upload_url: Option<String>,
    pub chunk_uploaded: bool,
    pub upload_complete: bool,
    pub next_chunk_index: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ModelListResponse {
    pub models: Vec<Model>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
    pub total_storage_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ModelDetailsResponse {
    pub model: Model,
    pub files: Vec<ModelFileInfo>,
    pub storage_size_bytes: u64,
    pub validation_issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub required_files: Vec<String>,
    pub present_files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelStorageInfo {
    pub provider_id: Uuid,
    pub total_models: i64,
    pub total_storage_bytes: u64,
    pub models_by_status: ModelStatusCounts,
}

#[derive(Debug, Serialize)]
pub struct ModelStatusCounts {
    pub active: i64,
    pub inactive: i64,
    pub deprecated: i64,
    pub enabled: i64,
    pub disabled: i64,
}

impl Model {
    /// Get the model path using the pattern {provider_id}/{id}
    pub fn get_model_path(&self) -> String {
        format!("models/{}/{}", self.provider_id, self.id)
    }

    pub fn get_model_absolute_path(&self) -> String {
        APP_DATA_DIR
            .join(self.get_model_path())
            .to_string_lossy()
            .to_string()
    }

    /// Set files from ModelFileDb structs
    pub fn with_files(mut self, files: Option<Vec<ModelFile>>) -> Self {
        self.files = files.map(|files| {
            files
                .into_iter()
                .map(|f| ModelFileInfo {
                    filename: f.filename,
                    file_size_bytes: f.file_size_bytes,
                    file_type: f.file_type,
                    uploaded_at: f.uploaded_at,
                })
                .collect()
        });
        self
    }

    /// Get the model settings, or return default settings if none are set
    pub fn get_settings(&self) -> ModelSettings {
        self.settings.clone().unwrap_or_default()
    }
}
