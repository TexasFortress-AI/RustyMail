// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/openai.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct
use crate::api::errors::ApiError as RestApiError;

// Get OpenAI API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
}

const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini"; 

// --- OpenAI Specific Request/Response Structs ---
#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    // Add other parameters like temperature, max_tokens if needed
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    // Add usage, error fields if needed
}

#[derive(Deserialize, Debug)]
struct OpenAiChoice {
    message: AiChatMessage,
    // Add other fields if needed, like finish_reason
}

#[derive(Deserialize, Debug)]
struct OpenAiUsage {
    // Define usage fields if needed
}

#[derive(Deserialize, Debug)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize, Debug)]
struct OpenAiModel {
    id: String,
    object: String,
}

#[derive(Clone)]
pub struct OpenAiAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl OpenAiAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| DEFAULT_OPENAI_MODEL.to_string()),
        }
    }

    // Optional: Allow setting a different model
    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for OpenAiAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from OpenAI API");
        let base_url = get_base_url();
        let models_url = format!("{}/models", base_url);

        let response = self.http_client
            .get(&models_url)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("OpenAI models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenAI models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("OpenAI models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OpenAiModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize OpenAI models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .filter(|model| model.object == "model")
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from OpenAI API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let chat_url = format!("{}/chat/completions", base_url);

        let request_payload = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(), // Clone messages for the request
        };

        debug!("Sending request to OpenAI API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), chat_url);

        let response = self.http_client
            .post(&chat_url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("OpenAI: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenAI API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("OpenAI API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize OpenAI response: {}", e) })?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from OpenAI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("OpenAI API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "OpenAI response was empty or missing choices".to_string() })
        }
    }
} 