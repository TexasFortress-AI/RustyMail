// src/dashboard/services/ai/providers/xai.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage};
use crate::api::errors::ApiError as RestApiError;

// Get xAI API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("XAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.x.ai/v1".to_string())
}

const DEFAULT_XAI_MODEL: &str = "grok-beta";

// --- xAI Specific Request/Response Structs (OpenAI-compatible) ---
#[derive(Serialize)]
struct XAIChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct XAIChatResponse {
    choices: Vec<XAIChoice>,
}

#[derive(Deserialize)]
struct XAIChoice {
    message: AiChatMessage,
}

#[derive(Deserialize)]
struct XAIModelsResponse {
    data: Vec<XAIModel>,
}

#[derive(Deserialize)]
struct XAIModel {
    id: String,
}

#[derive(Clone)]
pub struct XAIAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl XAIAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("XAI_MODEL")
                .unwrap_or_else(|_| DEFAULT_XAI_MODEL.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for XAIAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from xAI API");
        let base_url = get_base_url();
        let models_url = format!("{}/models", base_url);

        let response = self.http_client
            .get(&models_url)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("xAI models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("xAI models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("xAI models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<XAIModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize xAI models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from xAI API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/chat/completions", base_url);

        let request_payload = XAIChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to xAI API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("xAI: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("xAI API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("xAI API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<XAIChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize xAI response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from xAI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("xAI API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "xAI response was empty or missing choices".to_string() })
        }
    }
}
