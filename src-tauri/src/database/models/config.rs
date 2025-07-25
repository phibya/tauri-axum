use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

// Configuration table structure
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConfigurationDb {
    pub id: i32,
    pub name: String,
    pub value: Value,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}