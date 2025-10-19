// src/dashboard/services/ai/model_config.rs
// AI Model Configuration Management
// Handles database storage and retrieval of AI model settings

use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;
use log::{debug, error};
use crate::api::errors::ApiError;

/// AI Model Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfiguration {
    pub role: String,  // 'tool_calling' or 'drafting'
    pub provider: String,  // 'ollama', 'openai', 'anthropic', etc.
    pub model_name: String,  // e.g., 'qwen3:4b-q8_0', 'gemma3:27b-it-q8_0'
    pub base_url: Option<String>,  // Provider API base URL
    pub api_key: Option<String>,  // Optional API key
    pub additional_config: Option<String>,  // JSON for provider-specific settings
}

impl ModelConfiguration {
    /// Create a new model configuration
    pub fn new(role: impl Into<String>, provider: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            provider: provider.into(),
            model_name: model_name.into(),
            base_url: None,
            api_key: None,
            additional_config: None,
        }
    }

    /// Set the base URL
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set additional configuration as JSON
    pub fn with_additional_config(mut self, config: impl Into<String>) -> Self {
        self.additional_config = Some(config.into());
        self
    }
}

/// Get model configuration for a specific role
pub async fn get_model_config(pool: &SqlitePool, role: &str) -> Result<ModelConfiguration, ApiError> {
    debug!("Fetching model configuration for role: {}", role);

    let row = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT role, provider, model_name, base_url, api_key, additional_config
         FROM ai_model_configurations
         WHERE role = ?"
    )
    .bind(role)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Database error fetching model config for role {}: {}", role, e);
        ApiError::InternalError { message: format!("Failed to fetch model configuration: {}", e) }
    })?;

    match row {
        Some((role, provider, model_name, base_url, api_key, additional_config)) => {
            Ok(ModelConfiguration {
                role,
                provider,
                model_name,
                base_url,
                api_key,
                additional_config,
            })
        }
        None => {
            Err(ApiError::NotFound { resource: format!("Model configuration for role: {}", role) })
        }
    }
}

/// Set model configuration for a specific role
pub async fn set_model_config(pool: &SqlitePool, config: &ModelConfiguration) -> Result<(), ApiError> {
    debug!("Setting model configuration for role: {}", config.role);

    sqlx::query(
        "INSERT INTO ai_model_configurations (role, provider, model_name, base_url, api_key, additional_config)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(role) DO UPDATE SET
            provider = excluded.provider,
            model_name = excluded.model_name,
            base_url = excluded.base_url,
            api_key = excluded.api_key,
            additional_config = excluded.additional_config,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&config.role)
    .bind(&config.provider)
    .bind(&config.model_name)
    .bind(&config.base_url)
    .bind(&config.api_key)
    .bind(&config.additional_config)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Database error setting model config for role {}: {}", config.role, e);
        ApiError::InternalError { message: format!("Failed to set model configuration: {}", e) }
    })?;

    debug!("Successfully set model configuration for role: {}", config.role);
    Ok(())
}

/// Get all model configurations
pub async fn get_all_model_configs(pool: &SqlitePool) -> Result<Vec<ModelConfiguration>, ApiError> {
    debug!("Fetching all model configurations");

    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT role, provider, model_name, base_url, api_key, additional_config
         FROM ai_model_configurations
         ORDER BY role"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Database error fetching all model configs: {}", e);
        ApiError::InternalError { message: format!("Failed to fetch model configurations: {}", e) }
    })?;

    let configs = rows.into_iter().map(|(role, provider, model_name, base_url, api_key, additional_config)| {
        ModelConfiguration {
            role,
            provider,
            model_name,
            base_url,
            api_key,
            additional_config,
        }
    }).collect();

    Ok(configs)
}

/// Delete model configuration for a specific role
pub async fn delete_model_config(pool: &SqlitePool, role: &str) -> Result<(), ApiError> {
    debug!("Deleting model configuration for role: {}", role);

    let result = sqlx::query("DELETE FROM ai_model_configurations WHERE role = ?")
        .bind(role)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Database error deleting model config for role {}: {}", role, e);
            ApiError::InternalError { message: format!("Failed to delete model configuration: {}", e) }
        })?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound { resource: format!("Model configuration for role: {}", role) });
    }

    debug!("Successfully deleted model configuration for role: {}", role);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_configuration_builder() {
        let config = ModelConfiguration::new("tool_calling", "ollama", "qwen3:4b-q8_0")
            .with_base_url("http://localhost:11434")
            .with_api_key("test-key");

        assert_eq!(config.role, "tool_calling");
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.model_name, "qwen3:4b-q8_0");
        assert_eq!(config.base_url, Some("http://localhost:11434".to_string()));
        assert_eq!(config.api_key, Some("test-key".to_string()));
    }
}
