// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/ollama.rs
// Uses native Ollama API for full control over sampler settings

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error, info};
use super::{AiProvider, AiChatMessage, get_ai_request_timeout, get_ai_generation_timeout};
use crate::api::errors::ApiError as RestApiError;
use crate::dashboard::services::ai::sampler_config::SamplerConfig;

// No default model - must be provided via OLLAMA_MODEL environment variable

// Default sampler settings for tool-calling
const DEFAULT_TEMPERATURE: f32 = 0.7;
const DEFAULT_TOP_P: f32 = 1.0;
const DEFAULT_MIN_P: f32 = 0.01;  // llama.cpp default is 0.05, we use 0.01
const DEFAULT_REPEAT_PENALTY: f32 = 1.0;  // Disabled
const DEFAULT_NUM_CTX: u32 = 51200;  // 50k context window

/// Ollama-specific options for model generation
#[derive(Debug, Clone, Serialize, Default)]
pub struct OllamaOptions {
    /// Temperature for sampling (0.0 = deterministic, higher = more random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p (nucleus) sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling (0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Min-p sampling (filters tokens below min_p * max_probability)
    /// Recommended: 0.01 for tool-calling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    /// Repeat penalty (1.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,

    /// Context window size in tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,

    /// Disable thinking/reasoning mode for thinking models (GLM-4, Qwen, etc.)
    /// IMPORTANT: Set to false to prevent verbose internal reasoning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Seed for reproducibility (-1 = random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

/// Native Ollama API chat request
#[derive(Serialize)]
struct OllamaNativeChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

/// Ollama message format (same as OpenAI but explicit)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OllamaMessage {
    role: String,
    content: String,
}

impl From<&AiChatMessage> for OllamaMessage {
    fn from(msg: &AiChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
        }
    }
}

/// Native Ollama API response
#[derive(Deserialize, Debug)]
struct OllamaNativeChatResponse {
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
}

/// OpenAI-compatible response (for models endpoint)
#[derive(Deserialize, Debug)]
struct OllamaModelsResponse {
    data: Vec<OllamaModel>,
}

#[derive(Deserialize, Debug)]
struct OllamaModel {
    id: String,
    object: String,
}

#[derive(Clone)]
pub struct OllamaAdapter {
    base_url: String,
    http_client: Client,
    model: String,
    options: OllamaOptions,
}

impl OllamaAdapter {
    pub fn new(base_url: String, http_client: Client) -> Self {
        // Model MUST come from environment variable - no hardcoded default
        let model = std::env::var("OLLAMA_MODEL")
            .expect("OLLAMA_MODEL environment variable must be set");

        // Default options optimized for tool-calling with thinking disabled
        let options = OllamaOptions {
            temperature: Some(DEFAULT_TEMPERATURE),
            top_p: Some(DEFAULT_TOP_P),
            min_p: Some(DEFAULT_MIN_P),
            repeat_penalty: Some(DEFAULT_REPEAT_PENALTY),
            num_ctx: Some(DEFAULT_NUM_CTX),
            think: Some(false),  // ALWAYS disable thinking mode
            ..Default::default()
        };

        Self {
            base_url,
            http_client,
            model,
            options,
        }
    }

    /// Set a different model
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// Override default options
    #[allow(dead_code)]
    pub fn with_options(mut self, options: OllamaOptions) -> Self {
        // Merge options, keeping think=false unless explicitly set
        self.options = OllamaOptions {
            temperature: options.temperature.or(self.options.temperature),
            top_p: options.top_p.or(self.options.top_p),
            top_k: options.top_k.or(self.options.top_k),
            min_p: options.min_p.or(self.options.min_p),
            repeat_penalty: options.repeat_penalty.or(self.options.repeat_penalty),
            num_ctx: options.num_ctx.or(self.options.num_ctx),
            num_predict: options.num_predict.or(self.options.num_predict),
            think: options.think.or(Some(false)),  // Default to false
            stop: options.stop.or(self.options.stop),
            seed: options.seed.or(self.options.seed),
        };
        self
    }

    /// Set temperature
    #[allow(dead_code)]
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.options.temperature = Some(temp);
        self
    }

    /// Set context window size
    #[allow(dead_code)]
    pub fn with_num_ctx(mut self, num_ctx: u32) -> Self {
        self.options.num_ctx = Some(num_ctx);
        self
    }

    /// Convert SamplerConfig from database to OllamaOptions
    /// Uses effective_* methods which apply the fallback chain: DB > env > code defaults
    fn sampler_config_to_options(config: &SamplerConfig) -> OllamaOptions {
        OllamaOptions {
            temperature: Some(config.effective_temperature()),
            top_p: Some(config.effective_top_p()),
            top_k: config.top_k.map(|v| v as u32),
            min_p: Some(config.effective_min_p()),
            repeat_penalty: Some(config.effective_repeat_penalty()),
            num_ctx: Some(config.effective_num_ctx()),
            num_predict: config.max_tokens.map(|v| v as i32),
            think: Some(config.effective_think_mode()),
            stop: if config.stop_sequences.is_empty() {
                None
            } else {
                Some(config.stop_sequences.clone())
            },
            seed: None,
        }
    }
}

#[async_trait]
impl AiProvider for OllamaAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Ollama API");

        // Use OpenAI-compatible models endpoint (still works for listing)
        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .timeout(get_ai_request_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Ollama models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Ollama models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Ollama models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OllamaModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Ollama models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .filter(|model| model.object == "model")
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from Ollama API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        // Use NATIVE Ollama API for full control over options
        let url = format!("{}/api/chat", self.base_url);

        // Convert messages to Ollama format
        let ollama_messages: Vec<OllamaMessage> = messages.iter().map(OllamaMessage::from).collect();

        let request_payload = OllamaNativeChatRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            options: Some(self.options.clone()),
        };

        info!("Sending request to Ollama native API: base_url={}, model={}, messages_count={}, options={:?}",
              self.base_url, request_payload.model, request_payload.messages.len(), self.options);

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .timeout(get_ai_generation_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Ollama: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Ollama API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Ollama API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OllamaNativeChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Ollama response: {}", e) })?;

        if response_body.done {
            info!("Ollama response complete. Tokens: prompt={:?}, eval={:?}",
                  response_body.prompt_eval_count, response_body.eval_count);
            Ok(response_body.message.content)
        } else {
            warn!("Ollama API response was not marked as done");
            Ok(response_body.message.content)
        }
    }

    async fn generate_response_with_config(
        &self,
        messages: &[AiChatMessage],
        config: Option<&SamplerConfig>,
    ) -> Result<String, RestApiError> {
        // Use database config if provided, otherwise fall back to self.options
        let options = match config {
            Some(cfg) => {
                info!("Using sampler config from database for {}/{}", cfg.provider, cfg.model_name);
                Self::sampler_config_to_options(cfg)
            }
            None => {
                debug!("No sampler config provided, using default options");
                self.options.clone()
            }
        };

        // Use NATIVE Ollama API for full control over options
        let url = format!("{}/api/chat", self.base_url);

        // Convert messages to Ollama format
        let ollama_messages: Vec<OllamaMessage> = messages.iter().map(OllamaMessage::from).collect();

        let request_payload = OllamaNativeChatRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            options: Some(options.clone()),
        };

        info!("Sending request to Ollama native API with config: base_url={}, model={}, messages_count={}, options={:?}",
              self.base_url, request_payload.model, request_payload.messages.len(), options);

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .timeout(get_ai_generation_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Ollama: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Ollama API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Ollama API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OllamaNativeChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Ollama response: {}", e) })?;

        if response_body.done {
            info!("Ollama response complete. Tokens: prompt={:?}, eval={:?}",
                  response_body.prompt_eval_count, response_body.eval_count);
            Ok(response_body.message.content)
        } else {
            warn!("Ollama API response was not marked as done");
            Ok(response_body.message.content)
        }
    }
}
