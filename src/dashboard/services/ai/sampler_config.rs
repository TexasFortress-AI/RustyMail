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
use sqlx::{SqlitePool, FromRow};
use log::{debug, error, info};
use crate::api::errors::ApiError;

/// Database row for ai_sampler_configs table
/// Used internally for SQLx queries (derives FromRow)
#[derive(Debug, FromRow)]
struct SamplerConfigRow {
    id: i64,
    provider: String,
    model_name: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<i32>,
    min_p: Option<f64>,
    typical_p: Option<f64>,
    repeat_penalty: Option<f64>,
    num_ctx: Option<i32>,
    max_tokens: Option<i32>,
    think_mode: i32,
    stop_sequences: String,
    system_prompt: Option<String>,
    provider_options: String,
    description: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl SamplerConfigRow {
    /// Convert database row to SamplerConfig
    fn into_config(self) -> SamplerConfig {
        let stop_sequences: Vec<String> = serde_json::from_str(&self.stop_sequences).unwrap_or_default();
        let provider_options: serde_json::Value = serde_json::from_str(&self.provider_options)
            .unwrap_or_else(|_| serde_json::json!({}));

        SamplerConfig {
            id: Some(self.id),
            provider: self.provider,
            model_name: self.model_name,
            temperature: self.temperature.map(|v| v as f32),
            top_p: self.top_p.map(|v| v as f32),
            top_k: self.top_k,
            min_p: self.min_p.map(|v| v as f32),
            typical_p: self.typical_p.map(|v| v as f32),
            repeat_penalty: self.repeat_penalty.map(|v| v as f32),
            num_ctx: self.num_ctx.map(|v| v as u32),
            max_tokens: self.max_tokens.map(|v| v as u32),
            think_mode: self.think_mode != 0,
            stop_sequences,
            system_prompt: self.system_prompt,
            provider_options,
            description: self.description,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

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
    pub typical_p: Option<f32>,  // top-n-sigma / tail-free sampling
    pub repeat_penalty: Option<f32>,
    pub num_ctx: Option<u32>,
    pub max_tokens: Option<u32>,
    pub think_mode: bool,
    pub stop_sequences: Vec<String>,
    pub system_prompt: Option<String>,  // Custom system prompt override
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
            typical_p: None,
            repeat_penalty: None,
            num_ctx: None,
            max_tokens: None,
            think_mode: false,
            stop_sequences: Vec::new(),
            system_prompt: None,
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

        config.typical_p = std::env::var("SAMPLER_DEFAULT_TYPICAL_P")
            .ok()
            .and_then(|v| v.parse().ok());

        config.system_prompt = std::env::var("SAMPLER_DEFAULT_SYSTEM_PROMPT")
            .ok();

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
    let row = sqlx::query_as::<_, SamplerConfigRow>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, typical_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, system_prompt, provider_options,
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
        Some(db_row) => {
            info!("Found sampler config in database for {}/{}", provider, model_name);
            Ok(db_row.into_config())
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
            provider, model_name, temperature, top_p, top_k, min_p, typical_p, repeat_penalty,
            num_ctx, max_tokens, think_mode, stop_sequences, system_prompt, provider_options, description
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(provider, model_name) DO UPDATE SET
            temperature = excluded.temperature,
            top_p = excluded.top_p,
            top_k = excluded.top_k,
            min_p = excluded.min_p,
            typical_p = excluded.typical_p,
            repeat_penalty = excluded.repeat_penalty,
            num_ctx = excluded.num_ctx,
            max_tokens = excluded.max_tokens,
            think_mode = excluded.think_mode,
            stop_sequences = excluded.stop_sequences,
            system_prompt = excluded.system_prompt,
            provider_options = excluded.provider_options,
            description = excluded.description"
    )
    .bind(&config.provider)
    .bind(&config.model_name)
    .bind(config.temperature.map(|v| v as f64))
    .bind(config.top_p.map(|v| v as f64))
    .bind(config.top_k)
    .bind(config.min_p.map(|v| v as f64))
    .bind(config.typical_p.map(|v| v as f64))
    .bind(config.repeat_penalty.map(|v| v as f64))
    .bind(config.num_ctx.map(|v| v as i32))
    .bind(config.max_tokens.map(|v| v as i32))
    .bind(if config.think_mode { 1 } else { 0 })
    .bind(&stop_seq_json)
    .bind(&config.system_prompt)
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

    let rows = sqlx::query_as::<_, SamplerConfigRow>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, typical_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, system_prompt, provider_options,
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

    let configs = rows.into_iter().map(|row| row.into_config()).collect();
    Ok(configs)
}

/// List sampler configs for a specific provider
pub async fn list_sampler_configs_by_provider(
    pool: &SqlitePool,
    provider: &str,
) -> Result<Vec<SamplerConfig>, ApiError> {
    debug!("Listing sampler configs for provider: {}", provider);

    let rows = sqlx::query_as::<_, SamplerConfigRow>(
        "SELECT id, provider, model_name, temperature, top_p, top_k, min_p, typical_p, repeat_penalty,
                num_ctx, max_tokens, think_mode, stop_sequences, system_prompt, provider_options,
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

    let configs = rows.into_iter().map(|row| row.into_config()).collect();
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

// ============================================================================
// Sampler Configuration Presets
// ============================================================================
// Recommended configurations for popular models based on extensive testing.
// These can be imported into the database via the WebUI.

/// Preset category for grouping related presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCategory {
    pub name: String,
    pub description: String,
    pub presets: Vec<SamplerConfig>,
}

/// Get all recommended sampler configuration presets
pub fn get_recommended_presets() -> Vec<PresetCategory> {
    vec![
        PresetCategory {
            name: "Ollama - Recommended".to_string(),
            description: "Optimized settings for popular Ollama models".to_string(),
            presets: vec![
                create_preset("ollama", "hf.co/unsloth/GLM-4.7-Flash-GGUF:q8_0",
                    0.7, 1.0, None, Some(0.01), 1.0, 51200, false,
                    "GLM-4.7-Flash Q8 - Recommended for tool-calling with 50k context"),
                create_preset("ollama", "qwen2.5:7b",
                    0.7, 0.95, None, Some(0.05), 1.1, 32768, false,
                    "Qwen 2.5 7B - Fast local model for simple tasks"),
                create_preset("ollama", "qwen2.5:14b",
                    0.7, 0.95, None, Some(0.05), 1.1, 32768, false,
                    "Qwen 2.5 14B - Balanced performance"),
                create_preset("ollama", "qwen2.5:32b",
                    0.7, 0.95, None, Some(0.05), 1.1, 32768, false,
                    "Qwen 2.5 32B - Larger model for complex reasoning"),
                create_preset("ollama", "llama3.2:7b",
                    0.8, 0.9, None, Some(0.05), 1.15, 8192, false,
                    "Llama 3.2 7B - General purpose model"),
                create_preset("ollama", "llama3.3:70b",
                    0.7, 0.9, None, Some(0.05), 1.1, 131072, false,
                    "Llama 3.3 70B - Large model with 128k context"),
                create_preset("ollama", "mistral:7b",
                    0.7, 0.95, None, Some(0.05), 1.1, 32768, false,
                    "Mistral 7B - Fast reasoning model"),
            ],
        },
        PresetCategory {
            name: "llama.cpp - Recommended".to_string(),
            description: "Optimized settings for llama.cpp server".to_string(),
            presets: vec![
                create_preset("llamacpp", "default",
                    0.7, 1.0, None, Some(0.01), 1.0, 51200, false,
                    "Default llama.cpp settings - applies when no model-specific config exists"),
                create_preset("llamacpp", "GLM-4.7-Flash",
                    0.7, 1.0, None, Some(0.01), 1.0, 51200, false,
                    "GLM-4.7-Flash - Optimized for 50k context and tool-calling"),
            ],
        },
        PresetCategory {
            name: "Cloud Providers".to_string(),
            description: "Settings for OpenAI, Anthropic, and other cloud APIs".to_string(),
            presets: vec![
                create_preset("openai", "gpt-4o",
                    0.7, 1.0, None, None, 1.0, 128000, false,
                    "GPT-4o - OpenAI's latest multimodal model"),
                create_preset("openai", "gpt-4-turbo",
                    0.7, 1.0, None, None, 1.0, 128000, false,
                    "GPT-4 Turbo - Fast and capable"),
                create_preset("anthropic", "claude-3-opus",
                    0.7, 1.0, None, None, 1.0, 200000, false,
                    "Claude 3 Opus - Most capable Anthropic model"),
                create_preset("anthropic", "claude-3-sonnet",
                    0.7, 1.0, None, None, 1.0, 200000, false,
                    "Claude 3 Sonnet - Balanced performance"),
            ],
        },
    ]
}

/// Helper to create a preset configuration
fn create_preset(
    provider: &str,
    model_name: &str,
    temperature: f32,
    top_p: f32,
    top_k: Option<i32>,
    min_p: Option<f32>,
    repeat_penalty: f32,
    num_ctx: u32,
    think_mode: bool,
    description: &str,
) -> SamplerConfig {
    SamplerConfig {
        id: None,
        provider: provider.to_string(),
        model_name: model_name.to_string(),
        temperature: Some(temperature),
        top_p: Some(top_p),
        top_k,
        min_p,
        typical_p: None,
        repeat_penalty: Some(repeat_penalty),
        num_ctx: Some(num_ctx),
        max_tokens: None,
        think_mode,
        stop_sequences: Vec::new(),
        system_prompt: None,
        provider_options: serde_json::json!({}),
        description: Some(description.to_string()),
        created_at: None,
        updated_at: None,
    }
}

/// Import selected presets into the database
pub async fn import_presets(
    pool: &SqlitePool,
    presets: &[SamplerConfig],
    overwrite: bool,
) -> Result<ImportResult, ApiError> {
    let mut imported = 0;
    let mut skipped = 0;

    for preset in presets {
        // Check if config already exists
        let exists = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM ai_sampler_configs WHERE provider = ? AND model_name = ?"
        )
        .bind(&preset.provider)
        .bind(&preset.model_name)
        .fetch_one(pool)
        .await
        .unwrap_or(0) > 0;

        if exists && !overwrite {
            skipped += 1;
            continue;
        }

        match save_sampler_config(pool, preset).await {
            Ok(_) => imported += 1,
            Err(e) => {
                error!("Failed to import preset {}/{}: {:?}", preset.provider, preset.model_name, e);
                skipped += 1;
            }
        }
    }

    info!("Imported {} presets, skipped {}", imported, skipped);
    Ok(ImportResult { imported, skipped })
}

/// Result of preset import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
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
