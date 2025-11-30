// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/mistral.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use super::{AiProvider, AiChatMessage};
use crate::api::errors::ApiError as RestApiError;

// Get Mistral API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("MISTRAL_BASE_URL")
        .unwrap_or_else(|_| "https://api.mistral.ai/v1".to_string())
}

const DEFAULT_MISTRAL_MODEL: &str = "mistral-large-latest";

// --- Mistral Specific Request/Response Structs (OpenAI-compatible) ---
#[derive(Serialize)]
struct MistralChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct MistralChatResponse {
    choices: Vec<MistralChoice>,
}

#[derive(Deserialize)]
struct MistralChoice {
    message: AiChatMessage,
}

#[derive(Deserialize)]
struct MistralModelsResponse {
    data: Vec<MistralModel>,
}

#[derive(Deserialize)]
struct MistralModel {
    id: String,
}

#[derive(Clone)]
pub struct MistralAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl MistralAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("MISTRAL_MODEL")
                .unwrap_or_else(|_| DEFAULT_MISTRAL_MODEL.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for MistralAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Mistral API");
        let base_url = get_base_url();
        let models_url = format!("{}/models", base_url);

        let response = self.http_client
            .get(&models_url)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Mistral models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Mistral models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Mistral models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<MistralModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Mistral models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from Mistral API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/chat/completions", base_url);

        let request_payload = MistralChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to Mistral API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Mistral: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Mistral API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Mistral API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<MistralChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Mistral response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from Mistral API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("Mistral API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "Mistral response was empty or missing choices".to_string() })
        }
    }
}
