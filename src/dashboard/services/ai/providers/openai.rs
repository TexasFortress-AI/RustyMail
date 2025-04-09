// src/dashboard/services/ai/providers/openai.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct

// OpenAI API constants
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
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

#[derive(Deserialize)]
struct OpenAiChoice {
    message: AiChatMessage, // Reuses the common struct
    // Add finish_reason if needed
}
// --- End Structs ---

#[derive(Debug)]
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
            model: DEFAULT_OPENAI_MODEL.to_string(),
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
    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, ApiError> {
        let request_payload = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(), // Clone messages for the request
        };

        debug!("Sending request to OpenAI API: model={}, messages_count={}", 
               request_payload.model, request_payload.messages.len());

        let response = self.http_client
            .post(OPENAI_API_URL)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| ApiError::AiRequestError(format!("Network error calling OpenAI: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenAI API request failed with status {}: {}", status, error_body);
            return Err(ApiError::AiRequestError(format!(
                "OpenAI API returned error status {}: {}",
                status,
                error_body
            )));
        }

        let response_body = response
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|e| ApiError::AiServiceError(format!("Failed to deserialize OpenAI response: {}", e)))?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from OpenAI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("OpenAI API response did not contain any choices.");
            Err(ApiError::AiServiceError("OpenAI response was empty or missing choices".to_string()))
        }
    }
} 