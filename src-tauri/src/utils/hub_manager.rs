use crate::api::hub::*;
use crate::utils::hub_config::{get_hub_folder_path, HubConfig};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

// Global hub manager instance
pub static HUB_MANAGER: Lazy<Arc<Mutex<Option<HubManager>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub struct HubManager {
    pub config: HubConfig,
    app_data_dir: PathBuf,
}

impl HubManager {
    pub fn new(app_data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = HubConfig::load()?;
        Ok(Self {
            config,
            app_data_dir,
        })
    }

    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!(
            "Initializing hub manager for version {}",
            self.config.hub_version
        );

        // 1. Handle version migration if needed
        self.migrate_hub_version_if_needed().await?;

        // 2. Ensure current version directory exists
        let hub_dir = self.get_hub_data_dir();
        fs::create_dir_all(&hub_dir).await?;
        println!("Hub directory ensured: {}", hub_dir.display());

        // 3. Copy embedded files (with modification time check)
        self.copy_embedded_hub_files().await?;

        // 4. Validate that all required files exist and are readable
        self.validate_hub_files().await?;

        // 5. Check for updates from GitHub
        if self.should_check_for_updates().await? {
            println!("Checking for hub updates from GitHub...");
            if let Err(e) = self.update_hub_files_from_github().await {
                eprintln!("Failed to update hub files from GitHub: {}", e);
                println!("Continuing with existing files in APP_DATA_DIR");
            } else {
                println!("Hub files updated from GitHub");
            }
        } else {
            println!("Skipping GitHub update check (too recent)");
        }

        println!("Hub manager initialization completed");
        Ok(())
    }

    pub async fn load_hub_data(&self) -> Result<HubData, Box<dyn std::error::Error + Send + Sync>> {
        // Always read from APP_DATA_DIR
        self.load_hub_from_data_dir().await
    }

    pub async fn refresh_hub(&self) -> Result<HubData, Box<dyn std::error::Error + Send + Sync>> {
        // Force download latest files from GitHub to APP_DATA_DIR
        self.update_hub_files_from_github().await?;
        self.load_hub_from_data_dir().await
    }

    async fn copy_embedded_hub_files(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let hub_folder = get_hub_folder_path();
        let embedded_hub_dir = hub_folder.join(&self.config.hub_version);
        let data_hub_dir = self.get_hub_data_dir();

        println!(
            "Copying hub files from: {} to APP_DATA_DIR: {}",
            embedded_hub_dir.display(),
            data_hub_dir.display()
        );

        for filename in &self.config.hub_files {
            let embedded_file_path = embedded_hub_dir.join(filename);
            let data_file_path = data_hub_dir.join(filename);

            if embedded_file_path.exists() {
                let should_copy = if data_file_path.exists() {
                    // Compare modification times
                    self.should_copy_based_on_modified_time(&embedded_file_path, &data_file_path)
                        .await?
                } else {
                    // File doesn't exist, always copy
                    true
                };

                if should_copy {
                    // Read and validate embedded file
                    let content = fs::read_to_string(&embedded_file_path).await?;

                    // Validate JSON structure based on file type
                    match filename.as_str() {
                        "models.json" => {
                            let _: HubModelsFile = serde_json::from_str(&content)
                                .map_err(|e| format!("Invalid models.json structure: {}", e))?;
                        }
                        "assistants.json" => {
                            let _: HubAssistantsFile = serde_json::from_str(&content)
                                .map_err(|e| format!("Invalid assistants.json structure: {}", e))?;
                        }
                        _ => {
                            // Generic JSON validation
                            let _: serde_json::Value = serde_json::from_str(&content)
                                .map_err(|e| format!("Invalid JSON in {}: {}", filename, e))?;
                        }
                    }

                    // Write to APP_DATA_DIR
                    fs::write(&data_file_path, content).await?;

                    // Copy timestamps
                    self.copy_file_timestamps(&embedded_file_path, &data_file_path)
                        .await?;

                    println!("Copied/Updated {} from embedded to APP_DATA_DIR", filename);
                } else {
                    println!(
                        "Skipped {} - APP_DATA_DIR version is newer or same",
                        filename
                    );
                }
            } else {
                // Create empty files if embedded doesn't exist
                println!(
                    "Embedded file {} not found, creating empty file in APP_DATA_DIR",
                    filename
                );

                let empty_content = match filename.as_str() {
                    "models.json" => {
                        let empty_models = HubModelsFile {
                            hub_version: self.config.hub_version.clone(),
                            schema_version: 1,
                            models: vec![],
                        };
                        serde_json::to_string_pretty(&empty_models)?
                    }
                    "assistants.json" => {
                        let empty_assistants = HubAssistantsFile {
                            hub_version: self.config.hub_version.clone(),
                            schema_version: 1,
                            assistants: vec![],
                        };
                        serde_json::to_string_pretty(&empty_assistants)?
                    }
                    _ => "{}".to_string(),
                };

                fs::write(&data_file_path, empty_content).await?;
                println!("Created empty {} in APP_DATA_DIR", filename);
            }
        }

        // Update version marker
        let version_file = data_hub_dir.join("hub_version");
        fs::write(version_file, &self.config.hub_version).await?;

        Ok(())
    }

    async fn load_hub_from_data_dir(
        &self,
    ) -> Result<HubData, Box<dyn std::error::Error + Send + Sync>> {
        let hub_dir = self.get_hub_data_dir();

        // Load models
        let models_path = hub_dir.join("models.json");
        let models_content = fs::read_to_string(&models_path)
            .await
            .map_err(|e| format!("Failed to read models from APP_DATA_DIR: {}", e))?;
        let models_file: HubModelsFile = serde_json::from_str(&models_content)?;

        // Load assistants
        let assistants_path = hub_dir.join("assistants.json");
        let assistants_content = fs::read_to_string(&assistants_path)
            .await
            .map_err(|e| format!("Failed to read assistants from APP_DATA_DIR: {}", e))?;
        let assistants_file: HubAssistantsFile = serde_json::from_str(&assistants_content)?;

        // Get last_updated from file modification time (simplified)
        let last_updated_iso = "2024-01-01T00:00:00Z".to_string();

        Ok(HubData {
            models: models_file.models,
            assistants: assistants_file.assistants,
            hub_version: self.config.hub_version.clone(),
            last_updated: last_updated_iso,
        })
    }

    async fn update_hub_files_from_github(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let hub_dir = self.get_hub_data_dir();

        for filename in &self.config.hub_files {
            let url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}/{}",
                self.config.github_repo,
                self.config.github_branch,
                self.config.hub_version,
                filename
            );

            println!("Updating {} from GitHub: {}", filename, url);

            let response = client.get(&url).send().await?;
            let content = response.text().await?;

            // Validate JSON
            let _: serde_json::Value = serde_json::from_str(&content)?;

            // Write to APP_DATA_DIR (overwrite existing)
            let file_path = hub_dir.join(filename);
            fs::write(file_path, content).await?;

            println!("Updated {} in APP_DATA_DIR", filename);
        }

        // Update last check timestamp
        self.update_last_check_time().await?;
        Ok(())
    }

    async fn should_copy_based_on_modified_time(
        &self,
        embedded_path: &PathBuf,
        data_path: &PathBuf,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let embedded_metadata = fs::metadata(embedded_path).await?;
        let data_metadata = fs::metadata(data_path).await?;

        let embedded_modified = embedded_metadata.modified()?;
        let data_modified = data_metadata.modified()?;

        let should_copy = embedded_modified > data_modified;

        if should_copy {
            println!(
                "Embedded {} is newer - will copy (embedded: {:?}, data: {:?})",
                embedded_path.file_name().unwrap().to_string_lossy(),
                embedded_modified,
                data_modified
            );
        }

        Ok(should_copy)
    }

    async fn copy_file_timestamps(
        &self,
        source_path: &PathBuf,
        dest_path: &PathBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let source_metadata = fs::metadata(source_path).await?;
        let modified_time = source_metadata.modified()?;
        let file = std::fs::File::open(dest_path)?;
        file.set_modified(modified_time)?;
        Ok(())
    }

    fn get_hub_data_dir(&self) -> PathBuf {
        self.app_data_dir.join("hub").join(&self.config.hub_version)
    }

    async fn should_check_for_updates(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let last_check_file = self.get_hub_data_dir().join("last_check");

        if !last_check_file.exists() {
            return Ok(true);
        }

        let last_check_content = fs::read_to_string(last_check_file).await?;
        let last_check: u64 = last_check_content.parse()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let hours_since_check = (now - last_check) / 3600;
        Ok(hours_since_check >= self.config.update_check_interval_hours)
    }

    async fn update_last_check_time(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let last_check_file = self.get_hub_data_dir().join("last_check");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        fs::write(last_check_file, now.to_string()).await?;
        Ok(())
    }

    async fn validate_hub_files(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let hub_dir = self.get_hub_data_dir();

        for filename in &self.config.hub_files {
            let file_path = hub_dir.join(filename);

            if !file_path.exists() {
                return Err(
                    format!("Required hub file missing in APP_DATA_DIR: {}", filename).into(),
                );
            }

            // Validate JSON structure
            let content = fs::read_to_string(&file_path).await?;
            let _: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON in APP_DATA_DIR {}: {}", filename, e))?;
        }

        Ok(())
    }

    async fn migrate_hub_version_if_needed(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let hub_base_dir = self.app_data_dir.join("hub");
        let version_file = hub_base_dir.join("current_version");

        let current_version = if version_file.exists() {
            fs::read_to_string(&version_file).await?
        } else {
            String::new()
        };

        if current_version != self.config.hub_version {
            println!(
                "Hub version migration: '{}' -> '{}'",
                current_version, self.config.hub_version
            );

            // Create new version directory
            let new_hub_dir = self.get_hub_data_dir();
            fs::create_dir_all(&new_hub_dir).await?;

            // Copy embedded files for new version
            self.copy_embedded_hub_files().await?;

            // Update version marker
            fs::write(version_file, &self.config.hub_version).await?;

            println!("Hub migrated to version {}", self.config.hub_version);
        }

        Ok(())
    }
}
