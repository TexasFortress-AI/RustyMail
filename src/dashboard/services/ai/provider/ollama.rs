// src/dashboard/services/ai/providers/ollama.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct
use crate::api::errors::ApiError as RestApiError;

// Default Ollama model
const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

// --- Ollama Specific Request/Response Structs ---
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    choices: Vec<OllamaChoice>,
    // Add usage, error fields if needed
}

#[derive(Deserialize, Debug)]
struct OllamaChoice {
    message: AiChatMessage,
    // Add other fields if needed, like finish_reason
}

#[derive(Deserialize, Debug)]
struct OllamaModelsResponse {
    data: Vec<OllamaModel>,
}

#[derive(Deserialize, Debug)]
struct OllamaModel {
    id: String,
    object: String,
}

#[derive(Clone)]
pub struct OllamaAdapter {
    base_url: String,
    http_client: Client,
    model: String,
}

impl OllamaAdapter {
    pub fn new(base_url: String, http_client: Client) -> Self {
        Self {
            base_url,
            http_client,
            model: DEFAULT_OLLAMA_MODEL.to_string(),
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
impl AiProvider for OllamaAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Ollama API");

        // Use OpenAI-compatible models endpoint
        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Ollama models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Ollama models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Ollama models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OllamaModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Ollama models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .filter(|model| model.object == "model")
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from Ollama API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        // Use OpenAI-compatible endpoint
        let url = format!("{}/v1/chat/completions", self.base_url);

        let request_payload = OllamaChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(), // Clone messages for the request
            stream: false, // Disable streaming for now
        };

        debug!("Sending request to Ollama API: base_url={}, model={}, messages_count={}",
               self.base_url, request_payload.model, request_payload.messages.len());

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(60)) // Ollama might be slower
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Ollama: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Ollama API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Ollama API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OllamaChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Ollama response: {}", e) })?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from Ollama API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("Ollama API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "Ollama response was empty or missing choices".to_string() })
        }
    }
}