// src/dashboard/services/ai/providers/anthropic.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct
use crate::api::errors::ApiError as RestApiError;

// Get Anthropic API base URL from environment or use default
fn get_base_url() -> String {
    std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string())
}

const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-5";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// --- Anthropic Specific Request/Response Structs ---
#[derive(Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContent>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

// Note: Anthropic doesn't have a models list endpoint, so we'll provide a static list
const ANTHROPIC_MODELS: &[&str] = &[
    "claude-sonnet-4-5",
    "claude-3-5-sonnet-20241022",
    "claude-3-5-sonnet-20240620",
    "claude-3-opus-20240229",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
];

#[derive(Clone)]
pub struct AnthropicAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl AnthropicAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: std::env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| DEFAULT_ANTHROPIC_MODEL.to_string()),
        }
    }

    // Allow setting a different model
    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AiProvider for AnthropicAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        // Anthropic doesn't provide a models list endpoint, return static list
        debug!("Returning static list of Anthropic Claude models");
        Ok(ANTHROPIC_MODELS.iter().map(|s| s.to_string()).collect())
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let base_url = get_base_url();
        let url = format!("{}/messages", base_url);

        // Convert AiChatMessage to Anthropic format
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .map(|msg| AnthropicMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            })
            .collect();

        let request_payload = AnthropicMessagesRequest {
            model: self.model.clone(),
            messages: anthropic_messages,
            max_tokens: 2000,
            temperature: Some(0.7),
        };

        debug!("Sending request to Anthropic API: model={}, messages_count={}, url={}",
               request_payload.model, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Anthropic: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Anthropic API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Anthropic API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<AnthropicMessagesResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Anthropic response: {}", e) })?;

        // Extract the first content block's text
        if let Some(content) = response_body.content.first() {
            debug!("Received response from Anthropic API.");
            Ok(content.text.clone())
        } else {
            warn!("Anthropic API response did not contain any content blocks.");
            Err(RestApiError::UnprocessableEntity { message: "Anthropic response was empty or missing content".to_string() })
        }
    }
}
