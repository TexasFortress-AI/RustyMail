// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/lmstudio.rs
// LM Studio adapter - OpenAI-compatible local inference server

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error, info};
use super::{AiProvider, AiChatMessage, get_ai_request_timeout, get_ai_generation_timeout};
use crate::api::errors::ApiError as RestApiError;

// Default sampler settings for tool-calling (same as llama.cpp)
const DEFAULT_TEMPERATURE: f32 = 0.7;
const DEFAULT_TOP_P: f32 = 1.0;
const DEFAULT_MIN_P: f32 = 0.01;
const DEFAULT_REPEAT_PENALTY: f32 = 1.0;

/// LM Studio options for generation
/// Uses OpenAI-compatible API with some additional local parameters
#[derive(Debug, Clone, Serialize, Default)]
pub struct LmStudioOptions {
    /// Temperature for sampling (0.0 = deterministic, higher = more random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p (nucleus) sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling (0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Min-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    /// Repeat penalty (1.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,

    /// Frequency penalty (0.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (0.0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Seed for reproducibility (-1 = random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

/// LM Studio chat completion request (OpenAI-compatible)
#[derive(Serialize)]
struct LmStudioChatRequest {
    messages: Vec<LmStudioMessage>,
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
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    stream: bool,
}

/// LM Studio message format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LmStudioMessage {
    role: String,
    content: String,
}

impl From<&AiChatMessage> for LmStudioMessage {
    fn from(msg: &AiChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
        }
    }
}

/// LM Studio chat completion response
#[derive(Deserialize, Debug)]
struct LmStudioChatResponse {
    choices: Vec<LmStudioChoice>,
    #[serde(default)]
    usage: Option<LmStudioUsage>,
}

#[derive(Deserialize, Debug)]
struct LmStudioChoice {
    message: LmStudioMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct LmStudioUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    total_tokens: Option<u32>,
}

/// LM Studio models response
#[derive(Deserialize, Debug)]
struct LmStudioModelsResponse {
    data: Vec<LmStudioModel>,
}

#[derive(Deserialize, Debug)]
struct LmStudioModel {
    id: String,
}

#[derive(Clone)]
pub struct LmStudioAdapter {
    base_url: String,
    http_client: Client,
    options: LmStudioOptions,
}

impl LmStudioAdapter {
    pub fn new(base_url: String, http_client: Client) -> Self {
        // Default options optimized for tool-calling
        let options = LmStudioOptions {
            temperature: Some(DEFAULT_TEMPERATURE),
            top_p: Some(DEFAULT_TOP_P),
            min_p: Some(DEFAULT_MIN_P),
            repeat_penalty: Some(DEFAULT_REPEAT_PENALTY),
            ..Default::default()
        };

        Self {
            base_url,
            http_client,
            options,
        }
    }

    /// Override default options
    pub fn with_options(mut self, options: LmStudioOptions) -> Self {
        self.options = LmStudioOptions {
            temperature: options.temperature.or(self.options.temperature),
            top_p: options.top_p.or(self.options.top_p),
            top_k: options.top_k.or(self.options.top_k),
            min_p: options.min_p.or(self.options.min_p),
            repeat_penalty: options.repeat_penalty.or(self.options.repeat_penalty),
            frequency_penalty: options.frequency_penalty.or(self.options.frequency_penalty),
            presence_penalty: options.presence_penalty.or(self.options.presence_penalty),
            max_tokens: options.max_tokens.or(self.options.max_tokens),
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

    /// Set min-p sampling
    #[allow(dead_code)]
    pub fn with_min_p(mut self, min_p: f32) -> Self {
        self.options.min_p = Some(min_p);
        self
    }
}

#[async_trait]
impl AiProvider for LmStudioAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from LM Studio server");

        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .timeout(get_ai_request_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("LM Studio models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("LM Studio models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("LM Studio models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<LmStudioModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize LM Studio models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from LM Studio server", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Convert messages to LM Studio format
        let lmstudio_messages: Vec<LmStudioMessage> = messages.iter().map(LmStudioMessage::from).collect();

        let request_payload = LmStudioChatRequest {
            messages: lmstudio_messages,
            temperature: self.options.temperature,
            top_p: self.options.top_p,
            top_k: self.options.top_k,
            min_p: self.options.min_p,
            repeat_penalty: self.options.repeat_penalty,
            frequency_penalty: self.options.frequency_penalty,
            presence_penalty: self.options.presence_penalty,
            max_tokens: self.options.max_tokens,
            stop: self.options.stop.clone(),
            seed: self.options.seed,
            stream: false,
        };

        info!("Sending request to LM Studio server: base_url={}, messages_count={}, temp={:?}, min_p={:?}",
              self.base_url, request_payload.messages.len(), self.options.temperature, self.options.min_p);

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .timeout(get_ai_generation_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("LM Studio: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("LM Studio API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("LM Studio API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<LmStudioChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize LM Studio response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            if let Some(usage) = &response_body.usage {
                info!("LM Studio response complete. Tokens: prompt={:?}, completion={:?}, total={:?}",
                      usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
            }
            Ok(choice.message.content.clone())
        } else {
            warn!("LM Studio API response did not contain any choices");
            Err(RestApiError::UnprocessableEntity { message: "LM Studio response was empty or missing choices".to_string() })
        }
    }
}
