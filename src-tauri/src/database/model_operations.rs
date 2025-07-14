use crate::database::models::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct ModelOperations;

impl ModelOperations {
    /// Create a new uploaded model record
    pub async fn create_model(
        pool: &PgPool,
        request: &CreateUploadedModelRequest,
        model_path: &str,
    ) -> Result<UploadedModelDb, sqlx::Error> {
        let model_id = Uuid::new_v4();
        let now = Utc::now();
        
        let default_capabilities = serde_json::json!({
            "vision": false,
            "audio": false,
            "tools": false,
            "code_interpreter": false
        });
        
        let default_parameters = serde_json::json!({
            "max_tokens": 512,
            "temperature": 0.7,
            "top_p": 0.9,
            "repeat_penalty": 1.1,
            "repeat_last_n": 64
        });
        
        let capabilities = request.capabilities.clone().unwrap_or(default_capabilities);
        let parameters = request.parameters.clone().unwrap_or(default_parameters);
        
        let row = sqlx::query(
            r#"
            INSERT INTO uploaded_models (
                id, provider_id, name, alias, description, model_path, 
                architecture, quantization, file_size_bytes, enabled, 
                is_deprecated, is_active, capabilities, parameters, 
                validation_status, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            ) RETURNING id, provider_id, name, alias, description, model_path, 
                       architecture, quantization, file_size_bytes, checksum, enabled, 
                       is_deprecated, is_active, capabilities, parameters, 
                       validation_status, validation_issues, created_at, updated_at
            "#,
        )
        .bind(model_id)
        .bind(request.provider_id)
        .bind(&request.name)
        .bind(&request.alias)
        .bind(&request.description)
        .bind(model_path)
        .bind(&request.architecture)
        .bind(&request.quantization)
        .bind(0i64)
        .bind(false)
        .bind(false)
        .bind(false)
        .bind(&capabilities)
        .bind(&parameters)
        .bind("pending")
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;
        
        let model = UploadedModelDb {
            id: row.get("id"),
            provider_id: row.get("provider_id"),
            name: row.get("name"),
            alias: row.get("alias"),
            description: row.get("description"),
            model_path: row.get("model_path"),
            architecture: row.get("architecture"),
            quantization: row.get("quantization"),
            file_size_bytes: row.get("file_size_bytes"),
            checksum: row.get("checksum"),
            enabled: row.get("enabled"),
            is_deprecated: row.get("is_deprecated"),
            is_active: row.get("is_active"),
            capabilities: row.get("capabilities"),
            parameters: row.get("parameters"),
            validation_status: row.get("validation_status"),
            validation_issues: row.get("validation_issues"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };
        
        Ok(model)
    }
    
    /// Get model by ID
    pub async fn get_model_by_id(
        pool: &PgPool,
        model_id: &Uuid,
    ) -> Result<Option<UploadedModelDb>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, provider_id, name, alias, description, model_path, 
                    architecture, quantization, file_size_bytes, checksum, enabled, 
                    is_deprecated, is_active, capabilities, parameters, 
                    validation_status, validation_issues, created_at, updated_at
             FROM uploaded_models WHERE id = $1"
        )
        .bind(model_id)
        .fetch_optional(pool)
        .await?;
        
        if let Some(row) = row {
            let model = UploadedModelDb {
                id: row.get("id"),
                provider_id: row.get("provider_id"),
                name: row.get("name"),
                alias: row.get("alias"),
                description: row.get("description"),
                model_path: row.get("model_path"),
                architecture: row.get("architecture"),
                quantization: row.get("quantization"),
                file_size_bytes: row.get("file_size_bytes"),
                checksum: row.get("checksum"),
                enabled: row.get("enabled"),
                is_deprecated: row.get("is_deprecated"),
                is_active: row.get("is_active"),
                capabilities: row.get("capabilities"),
                parameters: row.get("parameters"),
                validation_status: row.get("validation_status"),
                validation_issues: row.get("validation_issues"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            Ok(Some(model))
        } else {
            Ok(None)
        }
    }
    
    /// List models for a provider with pagination
    pub async fn list_models_for_provider(
        pool: &PgPool,
        provider_id: &Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<UploadedModelDb>, i64), sqlx::Error> {
        let offset = (page - 1) * per_page;
        
        let rows = sqlx::query(
            r#"
            SELECT id, provider_id, name, alias, description, model_path, 
                   architecture, quantization, file_size_bytes, checksum, enabled, 
                   is_deprecated, is_active, capabilities, parameters, 
                   validation_status, validation_issues, created_at, updated_at
            FROM uploaded_models 
            WHERE provider_id = $1 
            ORDER BY created_at DESC 
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(provider_id)
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;
        
        let mut models = Vec::new();
        for row in rows {
            let model = UploadedModelDb {
                id: row.get("id"),
                provider_id: row.get("provider_id"),
                name: row.get("name"),
                alias: row.get("alias"),
                description: row.get("description"),
                model_path: row.get("model_path"),
                architecture: row.get("architecture"),
                quantization: row.get("quantization"),
                file_size_bytes: row.get("file_size_bytes"),
                checksum: row.get("checksum"),
                enabled: row.get("enabled"),
                is_deprecated: row.get("is_deprecated"),
                is_active: row.get("is_active"),
                capabilities: row.get("capabilities"),
                parameters: row.get("parameters"),
                validation_status: row.get("validation_status"),
                validation_issues: row.get("validation_issues"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            models.push(model);
        }
        
        let total_row = sqlx::query(
            "SELECT COUNT(*) as count FROM uploaded_models WHERE provider_id = $1"
        )
        .bind(provider_id)
        .fetch_one(pool)
        .await?;
        
        let total: i64 = total_row.get("count");
        
        Ok((models, total))
    }
    
    /// Update model validation status and issues
    pub async fn update_model_validation(
        pool: &PgPool,
        model_id: &Uuid,
        validation_status: &str,
        validation_issues: Option<&Vec<String>>,
        file_size_bytes: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        let issues_json = validation_issues
            .map(|issues| serde_json::to_value(issues).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        
        sqlx::query(
            r#"
            UPDATE uploaded_models 
            SET validation_status = $1, 
                validation_issues = $2, 
                file_size_bytes = COALESCE($3, file_size_bytes),
                updated_at = $4
            WHERE id = $5
            "#
        )
        .bind(validation_status)
        .bind(issues_json)
        .bind(file_size_bytes)
        .bind(Utc::now())
        .bind(model_id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Update model status (enabled/active)
    pub async fn update_model_status(
        pool: &PgPool,
        model_id: &Uuid,
        enabled: Option<bool>,
        is_active: Option<bool>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE uploaded_models 
            SET enabled = COALESCE($1, enabled),
                is_active = COALESCE($2, is_active),
                updated_at = $3
            WHERE id = $4
            "#
        )
        .bind(enabled)
        .bind(is_active)
        .bind(Utc::now())
        .bind(model_id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Update model checksum
    pub async fn update_model_checksum(
        pool: &PgPool,
        model_id: &Uuid,
        checksum: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE uploaded_models 
            SET checksum = $1, updated_at = $2
            WHERE id = $3
            "#
        )
        .bind(checksum)
        .bind(Utc::now())
        .bind(model_id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Delete model and its files
    pub async fn delete_model(
        pool: &PgPool,
        model_id: &Uuid,
    ) -> Result<(), sqlx::Error> {
        // Delete model files first (foreign key constraint)
        sqlx::query("DELETE FROM model_files WHERE model_id = $1")
            .bind(model_id)
            .execute(pool)
            .await?;
        
        // Delete the model
        sqlx::query("DELETE FROM uploaded_models WHERE id = $1")
            .bind(model_id)
            .execute(pool)
            .await?;
        
        Ok(())
    }
    
    /// Create a model file record
    pub async fn create_model_file(
        pool: &PgPool,
        model_id: &Uuid,
        filename: &str,
        file_path: &str,
        file_size_bytes: i64,
        file_type: &str,
        checksum: &str,
    ) -> Result<ModelFileDb, sqlx::Error> {
        let file_id = Uuid::new_v4();
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO model_files (
                id, model_id, filename, file_path, file_size_bytes, 
                file_type, checksum, upload_status, uploaded_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9
            ) RETURNING id, model_id, filename, file_path, file_size_bytes, 
                       file_type, checksum, upload_status, uploaded_at
            "#
        )
        .bind(file_id)
        .bind(model_id)
        .bind(filename)
        .bind(file_path)
        .bind(file_size_bytes)
        .bind(file_type)
        .bind(checksum)
        .bind("completed")
        .bind(now)
        .fetch_one(pool)
        .await?;
        
        let file = ModelFileDb {
            id: row.get("id"),
            model_id: row.get("model_id"),
            filename: row.get("filename"),
            file_path: row.get("file_path"),
            file_size_bytes: row.get("file_size_bytes"),
            file_type: row.get("file_type"),
            checksum: row.get("checksum"),
            upload_status: row.get("upload_status"),
            uploaded_at: row.get("uploaded_at"),
        };
        
        Ok(file)
    }
    
    /// Get files for a model
    pub async fn get_model_files(
        pool: &PgPool,
        model_id: &Uuid,
    ) -> Result<Vec<ModelFileDb>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, model_id, filename, file_path, file_size_bytes, 
                    file_type, checksum, upload_status, uploaded_at
             FROM model_files WHERE model_id = $1 ORDER BY uploaded_at ASC"
        )
        .bind(model_id)
        .fetch_all(pool)
        .await?;
        
        let mut files = Vec::new();
        for row in rows {
            let file = ModelFileDb {
                id: row.get("id"),
                model_id: row.get("model_id"),
                filename: row.get("filename"),
                file_path: row.get("file_path"),
                file_size_bytes: row.get("file_size_bytes"),
                file_type: row.get("file_type"),
                checksum: row.get("checksum"),
                upload_status: row.get("upload_status"),
                uploaded_at: row.get("uploaded_at"),
            };
            files.push(file);
        }
        
        Ok(files)
    }
    
    /// Get storage statistics for a provider
    pub async fn get_provider_storage_stats(
        pool: &PgPool,
        provider_id: &Uuid,
    ) -> Result<ModelStorageInfo, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_models,
                COALESCE(SUM(file_size_bytes), 0) as total_storage_bytes,
                COUNT(*) FILTER (WHERE is_active = true) as active,
                COUNT(*) FILTER (WHERE is_active = false) as inactive,
                COUNT(*) FILTER (WHERE is_deprecated = true) as deprecated,
                COUNT(*) FILTER (WHERE enabled = true) as enabled,
                COUNT(*) FILTER (WHERE enabled = false) as disabled
            FROM uploaded_models 
            WHERE provider_id = $1
            "#
        )
        .bind(provider_id)
        .fetch_one(pool)
        .await?;
        
        Ok(ModelStorageInfo {
            provider_id: *provider_id,
            total_models: row.get("total_models"),
            total_storage_bytes: row.get::<i64, _>("total_storage_bytes") as u64,
            models_by_status: ModelStatusCounts {
                active: row.get("active"),
                inactive: row.get("inactive"),
                deprecated: row.get("deprecated"),
                enabled: row.get("enabled"),
                disabled: row.get("disabled"),
            },
        })
    }
    
    /// Update model metadata
    pub async fn update_model(
        pool: &PgPool,
        model_id: &Uuid,
        request: &UpdateUploadedModelRequest,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE uploaded_models 
            SET name = COALESCE($1, name),
                alias = COALESCE($2, alias),
                description = COALESCE($3, description),
                enabled = COALESCE($4, enabled),
                capabilities = COALESCE($5, capabilities),
                parameters = COALESCE($6, parameters),
                updated_at = $7
            WHERE id = $8
            "#
        )
        .bind(&request.name)
        .bind(&request.alias)
        .bind(&request.description)
        .bind(request.enabled)
        .bind(&request.capabilities)
        .bind(&request.parameters)
        .bind(Utc::now())
        .bind(model_id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Get all models with their files for full response
    pub async fn get_model_with_files(
        pool: &PgPool,
        model_id: &Uuid,
    ) -> Result<Option<UploadedModel>, sqlx::Error> {
        let model_db = Self::get_model_by_id(pool, model_id).await?;
        
        if let Some(model_db) = model_db {
            let files = Self::get_model_files(pool, model_id).await?;
            Ok(Some(UploadedModel::from_db(model_db, files)))
        } else {
            Ok(None)
        }
    }
    
    /// List all models with their files for a provider
    pub async fn list_models_with_files_for_provider(
        pool: &PgPool,
        provider_id: &Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<UploadedModel>, i64), sqlx::Error> {
        let (model_dbs, total) = Self::list_models_for_provider(pool, provider_id, page, per_page).await?;
        
        let mut models = Vec::new();
        for model_db in model_dbs {
            let files = Self::get_model_files(pool, &model_db.id).await?;
            models.push(UploadedModel::from_db(model_db, files));
        }
        
        Ok((models, total))
    }
}