// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/llama_cpp.rs
// llama.cpp server adapter with support for advanced sampling (min-p, top-no, etc.)

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error, info};
use super::{AiProvider, AiChatMessage, get_ai_request_timeout, get_ai_generation_timeout};
use crate::api::errors::ApiError as RestApiError;

// Default sampler settings for tool-calling
const DEFAULT_TEMPERATURE: f32 = 0.7;
const DEFAULT_TOP_P: f32 = 1.0;
const DEFAULT_MIN_P: f32 = 0.01;  // llama.cpp default is 0.05, we use 0.01
const DEFAULT_REPEAT_PENALTY: f32 = 1.0;  // Disabled
const DEFAULT_N_CTX: u32 = 51200;  // 50k context window

/// llama.cpp server options for generation
/// Supports advanced sampling parameters not available in Ollama
#[derive(Debug, Clone, Serialize, Default)]
pub struct LlamaCppOptions {
    /// Temperature for sampling (0.0 = deterministic, higher = more random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p (nucleus) sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling (0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Min-p sampling (llama.cpp exclusive)
    /// Filters tokens with probability < min_p * max_probability
    /// Default in llama.cpp is 0.05, recommended 0.01 for tool-calling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    /// Top-no (tail-free sampling variant, llama.cpp exclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_no: Option<f32>,

    /// Typical-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typical_p: Option<f32>,

    /// Repeat penalty (1.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,

    /// Frequency penalty (0.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (0.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Maximum tokens to generate (-1 = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_predict: Option<i32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Seed for reproducibility (-1 = random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Mirostat mode (0 = disabled, 1 = mirostat, 2 = mirostat 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirostat: Option<u32>,

    /// Mirostat target entropy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirostat_tau: Option<f32>,

    /// Mirostat learning rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirostat_eta: Option<f32>,

    /// Grammar constraint (GBNF format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,

    /// JSON schema constraint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,

    /// Cache prompt for faster subsequent requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_prompt: Option<bool>,
}

/// llama.cpp chat completion request (OpenAI-compatible endpoint)
#[derive(Serialize)]
struct LlamaCppChatRequest {
    messages: Vec<LlamaCppMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_prompt: Option<bool>,
    stream: bool,
}

/// llama.cpp message format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LlamaCppMessage {
    role: String,
    content: String,
}

impl From<&AiChatMessage> for LlamaCppMessage {
    fn from(msg: &AiChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
        }
    }
}

/// llama.cpp chat completion response
#[derive(Deserialize, Debug)]
struct LlamaCppChatResponse {
    choices: Vec<LlamaCppChoice>,
    #[serde(default)]
    usage: Option<LlamaCppUsage>,
}

#[derive(Deserialize, Debug)]
struct LlamaCppChoice {
    message: LlamaCppMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct LlamaCppUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    total_tokens: Option<u32>,
}

/// llama.cpp models response
#[derive(Deserialize, Debug)]
struct LlamaCppModelsResponse {
    data: Vec<LlamaCppModel>,
}

#[derive(Deserialize, Debug)]
struct LlamaCppModel {
    id: String,
}

#[derive(Clone)]
pub struct LlamaCppAdapter {
    base_url: String,
    http_client: Client,
    options: LlamaCppOptions,
}

impl LlamaCppAdapter {
    pub fn new(base_url: String, http_client: Client) -> Self {
        // Default options optimized for tool-calling
        let options = LlamaCppOptions {
            temperature: Some(DEFAULT_TEMPERATURE),
            top_p: Some(DEFAULT_TOP_P),
            min_p: Some(DEFAULT_MIN_P),
            repeat_penalty: Some(DEFAULT_REPEAT_PENALTY),
            cache_prompt: Some(true),  // Enable prompt caching
            ..Default::default()
        };

        Self {
            base_url,
            http_client,
            options,
        }
    }

    /// Override default options
    pub fn with_options(mut self, options: LlamaCppOptions) -> Self {
        // Merge options
        self.options = LlamaCppOptions {
            temperature: options.temperature.or(self.options.temperature),
            top_p: options.top_p.or(self.options.top_p),
            top_k: options.top_k.or(self.options.top_k),
            min_p: options.min_p.or(self.options.min_p),
            top_no: options.top_no.or(self.options.top_no),
            typical_p: options.typical_p.or(self.options.typical_p),
            repeat_penalty: options.repeat_penalty.or(self.options.repeat_penalty),
            frequency_penalty: options.frequency_penalty.or(self.options.frequency_penalty),
            presence_penalty: options.presence_penalty.or(self.options.presence_penalty),
            n_predict: options.n_predict.or(self.options.n_predict),
            stop: options.stop.or(self.options.stop),
            seed: options.seed.or(self.options.seed),
            mirostat: options.mirostat.or(self.options.mirostat),
            mirostat_tau: options.mirostat_tau.or(self.options.mirostat_tau),
            mirostat_eta: options.mirostat_eta.or(self.options.mirostat_eta),
            grammar: options.grammar.or(self.options.grammar),
            json_schema: options.json_schema.or(self.options.json_schema),
            cache_prompt: options.cache_prompt.or(self.options.cache_prompt),
        };
        self
    }

    /// Set temperature
    #[allow(dead_code)]
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.options.temperature = Some(temp);
        self
    }

    /// Set min-p sampling
    #[allow(dead_code)]
    pub fn with_min_p(mut self, min_p: f32) -> Self {
        self.options.min_p = Some(min_p);
        self
    }

    /// Set top-no sampling
    #[allow(dead_code)]
    pub fn with_top_no(mut self, top_no: f32) -> Self {
        self.options.top_no = Some(top_no);
        self
    }
}

#[async_trait]
impl AiProvider for LlamaCppAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from llama.cpp server");

        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .timeout(get_ai_request_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("llama.cpp models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("llama.cpp models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("llama.cpp models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<LlamaCppModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize llama.cpp models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from llama.cpp server", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        // llama.cpp server uses OpenAI-compatible chat completions endpoint
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Convert messages to llama.cpp format
        let llama_messages: Vec<LlamaCppMessage> = messages.iter().map(LlamaCppMessage::from).collect();

        let request_payload = LlamaCppChatRequest {
            messages: llama_messages,
            temperature: self.options.temperature,
            top_p: self.options.top_p,
            top_k: self.options.top_k,
            min_p: self.options.min_p,
            repeat_penalty: self.options.repeat_penalty,
            n_predict: self.options.n_predict,
            stop: self.options.stop.clone(),
            seed: self.options.seed,
            cache_prompt: self.options.cache_prompt,
            stream: false,
        };

        info!("Sending request to llama.cpp server: base_url={}, messages_count={}, temp={:?}, min_p={:?}",
              self.base_url, request_payload.messages.len(), self.options.temperature, self.options.min_p);

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .timeout(get_ai_generation_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("llama.cpp: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("llama.cpp API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("llama.cpp API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<LlamaCppChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize llama.cpp response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            if let Some(usage) = &response_body.usage {
                info!("llama.cpp response complete. Tokens: prompt={:?}, completion={:?}, total={:?}",
                      usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
            }
            Ok(choice.message.content.clone())
        } else {
            warn!("llama.cpp API response did not contain any choices");
            Err(RestApiError::UnprocessableEntity { message: "llama.cpp response was empty or missing choices".to_string() })
        }
    }
}
