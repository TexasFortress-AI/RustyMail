// src/dashboard/services/ai/providers/deepseek.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage};
use crate::api::errors::ApiError as RestApiError;

// Get DeepSeek API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("DEEPSEEK_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string())
}

const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

// --- DeepSeek Specific Request/Response Structs (OpenAI-compatible) ---
#[derive(Serialize)]
struct DeepSeekChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct DeepSeekChatResponse {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Deserialize)]
struct DeepSeekChoice {
    message: AiChatMessage,
}

#[derive(Deserialize)]
struct DeepSeekModelsResponse {
    data: Vec<DeepSeekModel>,
}

#[derive(Deserialize)]
struct DeepSeekModel {
    id: String,
}

#[derive(Clone)]
pub struct DeepSeekAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl DeepSeekAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("DEEPSEEK_MODEL")
                .unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for DeepSeekAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from DeepSeek API");
        let base_url = get_base_url();
        let models_url = format!("{}/models", base_url);

        let response = self.http_client
            .get(&models_url)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("DeepSeek models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("DeepSeek models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("DeepSeek models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<DeepSeekModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize DeepSeek models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from DeepSeek API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/chat/completions", base_url);

        let request_payload = DeepSeekChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to DeepSeek API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("DeepSeek: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("DeepSeek API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("DeepSeek API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<DeepSeekChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize DeepSeek response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from DeepSeek API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("DeepSeek API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "DeepSeek response was empty or missing choices".to_string() })
        }
    }
}
