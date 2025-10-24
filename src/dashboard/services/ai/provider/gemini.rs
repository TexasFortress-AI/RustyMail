// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/gemini.rs
// Google Gemini uses a different API structure than OpenAI

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage};
use crate::api::errors::ApiError as RestApiError;

// Get Gemini API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("GEMINI_BASE_URL")
        .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1".to_string())
}

const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

// --- Gemini Specific Request/Response Structs ---
#[derive(Serialize)]
struct GeminiGenerateRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    max_output_tokens: usize,
}

#[derive(Deserialize)]
struct GeminiGenerateResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
}

#[derive(Clone)]
pub struct GeminiAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl GeminiAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    // Convert AiChatMessage role to Gemini role format
    fn convert_role(role: &str) -> String {
        match role {
            "assistant" => "model".to_string(),
            "system" => "user".to_string(), // Gemini doesn't have system role
            _ => role.to_string(),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Gemini API");
        let base_url = get_base_url();
        let models_url = format!("{}/models?key={}", base_url, self.api_key);

        let response = self.http_client
            .get(&models_url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Gemini models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Gemini models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Gemini models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<GeminiModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Gemini models response: {}", e) })?;

        // Extract model names (remove "models/" prefix)
        let models: Vec<String> = response_body.models
            .into_iter()
            .filter_map(|model| {
                model.name.strip_prefix("models/").map(|s| s.to_string())
            })
            .filter(|name| name.starts_with("gemini")) // Only include Gemini models
            .collect();

        debug!("Retrieved {} models from Gemini API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/models/{}:generateContent?key={}", base_url, self.model, self.api_key);

        // Convert AiChatMessage to Gemini format
        let contents: Vec<GeminiContent> = messages
            .iter()
            .map(|msg| GeminiContent {
                role: Self::convert_role(&msg.role),
                parts: vec![GeminiPart { text: msg.content.clone() }],
            })
            .collect();

        let request_payload = GeminiGenerateRequest {
            contents,
            generation_config: Some(GeminiGenerationConfig {
                temperature: 0.7,
                max_output_tokens: 2000,
            }),
        };

        debug!("Sending request to Gemini API: model={}, messages_count={}, url={}",
               self.model, request_payload.contents.len(), url);

        let response = self.http_client
            .post(&url)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Gemini: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Gemini API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Gemini API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<GeminiGenerateResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Gemini response: {}", e) })?;

        // Extract the first candidate's first part's text
        if let Some(candidate) = response_body.candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                debug!("Received response from Gemini API.");
                return Ok(part.text.clone());
            }
        }

        warn!("Gemini API response did not contain any candidates or parts.");
        Err(RestApiError::UnprocessableEntity { message: "Gemini response was empty or missing candidates".to_string() })
    }
}
