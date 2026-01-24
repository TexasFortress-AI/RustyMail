// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/sampler_config.rs
// AI Sampler Configuration Management
// Handles per-model sampler settings with layered configuration:
//   1. Database (per-model overrides via Web UI) - highest priority
//   2. Environment variables (deployment-time defaults)
//   3. Code defaults (fallback)

use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;
use log::{debug, error, info};
use crate::api::errors::ApiError;

/// Default sampler values (code fallbacks)
/// These are only used when both DB and env vars are missing
mod defaults {
    pub const TEMPERATURE: f32 = 0.7;
    pub const TOP_P: f32 = 1.0;
    pub const MIN_P: f32 = 0.01;
    pub const REPEAT_PENALTY: f32 = 1.0;
    pub const NUM_CTX: u32 = 8192;
    pub const THINK_MODE: bool = false;
}

/// AI Sampler Configuration for a specific model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplerConfig {
    pub id: Option<i64>,
    pub provider: String,
    pub model_name: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub num_ctx: Option<u32>,
    pub max_tokens: Option<u32>,
    pub think_mode: bool,
    pub stop_sequences: Vec<String>,
    pub provider_options: serde_json::Value,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl SamplerConfig {
    /// Create a new sampler config with provider and model
    pub fn new(provider: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            id: None,
            provider: provider.into(),
            model_name: model_name.into(),
            temperature: None,
            top_p: None,
            top_k: None,
            min_p: None,
            repeat_penalty: None,
            num_ctx: None,
            max_tokens: None,
            think_mode: false,
            stop_sequences: Vec::new(),
            provider_options: serde_json::json!({}),
            description: None,
            created_at: None,
            updated_at: None,
        }
    }

    /// Load defaults from environment variables
    pub fn from_env_defaults(provider: impl Into<String>, model_name: impl Into<String>) -> Self {
        let mut config = Self::new(provider, model_name);

        // Load from environment variables with code fallbacks
        config.temperature = std::env::var("SAMPLER_DEFAULT_TEMPERATURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(defaults::TEMPERATURE));

        config.top_p = std::env::var("SAMPLER_DEFAULT_TOP_P")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(defaults::TOP_P));

        config.top_k = std::env::var("SAMPLER_DEFAULT_TOP_K")
            .ok()
            .and_then(|v| v.parse().ok());

        config.min_p = std::env::var("SAMPLER_DEFAULT_MIN_P")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(defaults::MIN_P));

        config.repeat_penalty = std::env::var("SAMPLER_DEFAULT_REPEAT_PENALTY")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(defaults::REPEAT_PENALTY));

        config.num_ctx = std::env::var("SAMPLER_DEFAULT_NUM_CTX")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(defaults::NUM_CTX));

        config.max_tokens = std::env::var("SAMPLER_DEFAULT_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok());

        config.think_mode = std::env::var("SAMPLER_DEFAULT_THINK_MODE")
            .ok()
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(defaults::THINK_MODE);

        config
    }

    /// Get effective temperature (with fallback chain)
    pub fn effective_temperature(&self) -> f32 {
        self.temperature.unwrap_or_else(|| {
            std::env::var("SAMPLER_DEFAULT_TEMPERATURE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults::TEMPERATURE)
        })
    }

    /// Get effective top_p (with fallback chain)
    pub fn effective_top_p(&self) -> f32 {
        self.top_p.unwrap_or_else(|| {
            std::env::var("SAMPLER_DEFAULT_TOP_P")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults::TOP_P)
        })
    }

    /// Get effective min_p (with fallback chain)
    pub fn effective_min_p(&self) -> f32 {
        self.min_p.unwrap_or_else(|| {
            std::env::var("SAMPLER_DEFAULT_MIN_P")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults::MIN_P)
        })
    }

    /// Get effective repeat_penalty (with fallback chain)
    pub fn effective_repeat_penalty(&self) -> f32 {
        self.repeat_penalty.unwrap_or_else(|| {
            std::env::var("SAMPLER_DEFAULT_REPEAT_PENALTY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults::REPEAT_PENALTY)
        })
    }

    /// Get effective num_ctx (with fallback chain)
    pub fn effective_num_ctx(&self) -> u32 {
        self.num_ctx.unwrap_or_else(|| {
            std::env::var("SAMPLER_DEFAULT_NUM_CTX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(defaults::NUM_CTX)
        })
    }

    /// Get effective think_mode (with fallback chain)
    pub fn effective_think_mode(&self) -> bool {
        // For think_mode, DB value takes precedence
        // (think_mode field is never None, it's a bool with default false)
        self.think_mode
    }
}

/// Get sampler config for a specific provider and model
/// Returns config from DB if exists, otherwise returns env-based defaults
pub async fn get_sampler_config(
    pool: &SqlitePool,
    provider: &str,
    model_name: &str,
) -> Result<SamplerConfig, ApiError> {
    debug!("Fetching sampler config for provider: {}, model: {}", provider, model_name);

    // Try to get from database first
    let row = sqlx::query_as::<_, (
        i64,           // id
        String,        // provider
        String,        // model_name
        Option<f64>,   // temperature (REAL in SQLite)
        Option<f64>,   // top_p
        Option<i32>,   // top_k
        Option<f64>,   // min_p
        Option<f64>,   // repeat_penalty
        Option<i32>,   // num_ctx
        Option<i32>,   // max_tokens
        i32,           // think_mode (INTEGER in SQLite)
        String,        // stop_sequences
        String,        // provider_options
        Option<String>,// description
        Option<String>,// created_at
        Option<String>,// updated_at
    )>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, provider_options,
                description, created_at, updated_at
         FROM ai_sampler_configs
         WHERE provider = ? AND model_name = ?"
    )
    .bind(provider)
    .bind(model_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Database error fetching sampler config: {}", e);
        ApiError::InternalError { message: format!("Failed to fetch sampler config: {}", e) }
    })?;

    match row {
        Some((id, prov, model, temp, top_p, top_k, min_p, repeat_pen, num_ctx, max_tok, think, stop_seq, prov_opts, desc, created, updated)) => {
            info!("Found sampler config in database for {}/{}", provider, model_name);

            // Parse stop_sequences JSON
            let stop_sequences: Vec<String> = serde_json::from_str(&stop_seq).unwrap_or_default();

            // Parse provider_options JSON
            let provider_options: serde_json::Value = serde_json::from_str(&prov_opts)
                .unwrap_or_else(|_| serde_json::json!({}));

            Ok(SamplerConfig {
                id: Some(id),
                provider: prov,
                model_name: model,
                temperature: temp.map(|v| v as f32),
                top_p: top_p.map(|v| v as f32),
                top_k,
                min_p: min_p.map(|v| v as f32),
                repeat_penalty: repeat_pen.map(|v| v as f32),
                num_ctx: num_ctx.map(|v| v as u32),
                max_tokens: max_tok.map(|v| v as u32),
                think_mode: think != 0,
                stop_sequences,
                provider_options,
                description: desc,
                created_at: created,
                updated_at: updated,
            })
        }
        None => {
            debug!("No sampler config in DB for {}/{}, using env defaults", provider, model_name);
            Ok(SamplerConfig::from_env_defaults(provider, model_name))
        }
    }
}

/// Save sampler config to database
/// Uses UPSERT to insert or update existing config
pub async fn save_sampler_config(pool: &SqlitePool, config: &SamplerConfig) -> Result<i64, ApiError> {
    debug!("Saving sampler config for provider: {}, model: {}", config.provider, config.model_name);

    // Serialize stop_sequences to JSON
    let stop_seq_json = serde_json::to_string(&config.stop_sequences)
        .unwrap_or_else(|_| "[]".to_string());

    // Serialize provider_options to JSON
    let prov_opts_json = serde_json::to_string(&config.provider_options)
        .unwrap_or_else(|_| "{}".to_string());

    let result = sqlx::query(
        "INSERT INTO ai_sampler_configs (
            provider, model_name, temperature, top_p, top_k, min_p, repeat_penalty,
            num_ctx, max_tokens, think_mode, stop_sequences, provider_options, description
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(provider, model_name) DO UPDATE SET
            temperature = excluded.temperature,
            top_p = excluded.top_p,
            top_k = excluded.top_k,
            min_p = excluded.min_p,
            repeat_penalty = excluded.repeat_penalty,
            num_ctx = excluded.num_ctx,
            max_tokens = excluded.max_tokens,
            think_mode = excluded.think_mode,
            stop_sequences = excluded.stop_sequences,
            provider_options = excluded.provider_options,
            description = excluded.description"
    )
    .bind(&config.provider)
    .bind(&config.model_name)
    .bind(config.temperature.map(|v| v as f64))
    .bind(config.top_p.map(|v| v as f64))
    .bind(config.top_k)
    .bind(config.min_p.map(|v| v as f64))
    .bind(config.repeat_penalty.map(|v| v as f64))
    .bind(config.num_ctx.map(|v| v as i32))
    .bind(config.max_tokens.map(|v| v as i32))
    .bind(if config.think_mode { 1 } else { 0 })
    .bind(&stop_seq_json)
    .bind(&prov_opts_json)
    .bind(&config.description)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Database error saving sampler config: {}", e);
        ApiError::InternalError { message: format!("Failed to save sampler config: {}", e) }
    })?;

    info!("Saved sampler config for {}/{}", config.provider, config.model_name);
    Ok(result.last_insert_rowid())
}

/// List all sampler configs from database
pub async fn list_sampler_configs(pool: &SqlitePool) -> Result<Vec<SamplerConfig>, ApiError> {
    debug!("Listing all sampler configs");

    let rows = sqlx::query_as::<_, (
        i64, String, String, Option<f64>, Option<f64>, Option<i32>,
        Option<f64>, Option<f64>, Option<i32>, Option<i32>, i32,
        String, String, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, provider_options,
                description, created_at, updated_at
         FROM ai_sampler_configs
         ORDER BY provider, model_name"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Database error listing sampler configs: {}", e);
        ApiError::InternalError { message: format!("Failed to list sampler configs: {}", e) }
    })?;

    let configs = rows.into_iter().map(|(id, prov, model, temp, top_p, top_k, min_p, repeat_pen, num_ctx, max_tok, think, stop_seq, prov_opts, desc, created, updated)| {
        let stop_sequences: Vec<String> = serde_json::from_str(&stop_seq).unwrap_or_default();
        let provider_options: serde_json::Value = serde_json::from_str(&prov_opts)
            .unwrap_or_else(|_| serde_json::json!({}));

        SamplerConfig {
            id: Some(id),
            provider: prov,
            model_name: model,
            temperature: temp.map(|v| v as f32),
            top_p: top_p.map(|v| v as f32),
            top_k,
            min_p: min_p.map(|v| v as f32),
            repeat_penalty: repeat_pen.map(|v| v as f32),
            num_ctx: num_ctx.map(|v| v as u32),
            max_tokens: max_tok.map(|v| v as u32),
            think_mode: think != 0,
            stop_sequences,
            provider_options,
            description: desc,
            created_at: created,
            updated_at: updated,
        }
    }).collect();

    Ok(configs)
}

/// List sampler configs for a specific provider
pub async fn list_sampler_configs_by_provider(
    pool: &SqlitePool,
    provider: &str,
) -> Result<Vec<SamplerConfig>, ApiError> {
    debug!("Listing sampler configs for provider: {}", provider);

    let rows = sqlx::query_as::<_, (
        i64, String, String, Option<f64>, Option<f64>, Option<i32>,
        Option<f64>, Option<f64>, Option<i32>, Option<i32>, i32,
        String, String, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, provider_options,
                description, created_at, updated_at
         FROM ai_sampler_configs
         WHERE provider = ?
         ORDER BY model_name"
    )
    .bind(provider)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Database error listing sampler configs by provider: {}", e);
        ApiError::InternalError { message: format!("Failed to list sampler configs: {}", e) }
    })?;

    let configs = rows.into_iter().map(|(id, prov, model, temp, top_p, top_k, min_p, repeat_pen, num_ctx, max_tok, think, stop_seq, prov_opts, desc, created, updated)| {
        let stop_sequences: Vec<String> = serde_json::from_str(&stop_seq).unwrap_or_default();
        let provider_options: serde_json::Value = serde_json::from_str(&prov_opts)
            .unwrap_or_else(|_| serde_json::json!({}));

        SamplerConfig {
            id: Some(id),
            provider: prov,
            model_name: model,
            temperature: temp.map(|v| v as f32),
            top_p: top_p.map(|v| v as f32),
            top_k,
            min_p: min_p.map(|v| v as f32),
            repeat_penalty: repeat_pen.map(|v| v as f32),
            num_ctx: num_ctx.map(|v| v as u32),
            max_tokens: max_tok.map(|v| v as u32),
            think_mode: think != 0,
            stop_sequences,
            provider_options,
            description: desc,
            created_at: created,
            updated_at: updated,
        }
    }).collect();

    Ok(configs)
}

/// Delete a sampler config by provider and model
pub async fn delete_sampler_config(
    pool: &SqlitePool,
    provider: &str,
    model_name: &str,
) -> Result<(), ApiError> {
    debug!("Deleting sampler config for provider: {}, model: {}", provider, model_name);

    let result = sqlx::query(
        "DELETE FROM ai_sampler_configs WHERE provider = ? AND model_name = ?"
    )
    .bind(provider)
    .bind(model_name)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Database error deleting sampler config: {}", e);
        ApiError::InternalError { message: format!("Failed to delete sampler config: {}", e) }
    })?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound {
            resource: format!("Sampler config for {}/{}", provider, model_name)
        });
    }

    info!("Deleted sampler config for {}/{}", provider, model_name);
    Ok(())
}

/// Get current environment defaults (for display in UI)
pub fn get_env_defaults() -> SamplerConfig {
    SamplerConfig::from_env_defaults("env", "defaults")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_config_new() {
        let config = SamplerConfig::new("ollama", "test-model");
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.model_name, "test-model");
        assert!(config.temperature.is_none());
        assert!(!config.think_mode);
    }

    #[test]
    fn test_sampler_config_from_env_defaults() {
        // This test uses code defaults since env vars aren't set
        let config = SamplerConfig::from_env_defaults("ollama", "test-model");
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.effective_temperature(), defaults::TEMPERATURE);
        assert_eq!(config.effective_top_p(), defaults::TOP_P);
        assert_eq!(config.effective_min_p(), defaults::MIN_P);
        assert_eq!(config.effective_repeat_penalty(), defaults::REPEAT_PENALTY);
        assert_eq!(config.effective_num_ctx(), defaults::NUM_CTX);
    }

    #[test]
    fn test_effective_values_with_overrides() {
        let mut config = SamplerConfig::new("ollama", "test-model");
        config.temperature = Some(0.9);
        config.num_ctx = Some(32000);

        assert_eq!(config.effective_temperature(), 0.9);
        assert_eq!(config.effective_num_ctx(), 32000);
        // Other values should still use defaults
        assert_eq!(config.effective_top_p(), defaults::TOP_P);
    }
}
