// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/together.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use super::{AiProvider, AiChatMessage, get_ai_request_timeout};
use crate::api::errors::ApiError as RestApiError;

// Get Together AI API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("TOGETHER_BASE_URL")
        .unwrap_or_else(|_| "https://api.together.xyz/v1".to_string())
}

const DEFAULT_TOGETHER_MODEL: &str = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo";

// --- Together AI Specific Request/Response Structs (OpenAI-compatible) ---
#[derive(Serialize)]
struct TogetherChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct TogetherChatResponse {
    choices: Vec<TogetherChoice>,
}

#[derive(Deserialize)]
struct TogetherChoice {
    message: AiChatMessage,
}

#[derive(Deserialize)]
struct TogetherModelsResponse {
    data: Vec<TogetherModel>,
}

#[derive(Deserialize)]
struct TogetherModel {
    id: String,
}

#[derive(Clone)]
pub struct TogetherAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl TogetherAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("TOGETHER_MODEL")
                .unwrap_or_else(|_| DEFAULT_TOGETHER_MODEL.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for TogetherAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Together AI API");
        let base_url = get_base_url();
        let models_url = format!("{}/models", base_url);

        let response = self.http_client
            .get(&models_url)
            .bearer_auth(&self.api_key)
            .timeout(get_ai_request_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Together AI models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Together AI models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Together AI models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<TogetherModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Together AI models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from Together AI API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/chat/completions", base_url);

        let request_payload = TogetherChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to Together AI API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(get_ai_request_timeout())
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Together AI: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Together AI API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Together AI API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<TogetherChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Together AI response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from Together AI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("Together AI API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "Together AI response was empty or missing choices".to_string() })
        }
    }
}
