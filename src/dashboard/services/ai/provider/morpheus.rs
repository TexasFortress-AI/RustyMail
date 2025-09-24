// src/dashboard/services/ai/providers/morpheus.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct
use crate::api::errors::ApiError as RestApiError;

// Morpheus API constants
const MORPHEUS_API_BASE_URL: &str = "https://api.dev.mor.org/v1";
const MORPHEUS_MODELS_URL: &str = "https://api.dev.mor.org/v1/models";
const DEFAULT_MORPHEUS_MODEL: &str = "llama-3.2-90b-vision-instruct";

// --- Morpheus Specific Request/Response Structs ---
#[derive(Serialize)]
struct MorpheusChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct MorpheusChatResponse {
    choices: Vec<MorpheusChoice>,
    // Add usage, error fields if needed
}

#[derive(Deserialize, Debug)]
struct MorpheusChoice {
    message: AiChatMessage,
    // Add other fields if needed, like finish_reason
}

#[derive(Deserialize, Debug)]
struct MorpheusModelsResponse {
    data: Vec<MorpheusModel>,
}

#[derive(Deserialize, Debug)]
struct MorpheusModel {
    id: String,
    object: String,
}

#[derive(Clone)]
pub struct MorpheusAdapter {
    api_key: String,
    http_client: Client,
    model: String,
}

impl MorpheusAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        Self {
            api_key,
            http_client,
            model: DEFAULT_MORPHEUS_MODEL.to_string(),
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
impl AiProvider for MorpheusAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from Morpheus API");

        let response = self.http_client
            .get(MORPHEUS_MODELS_URL)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Morpheus models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Morpheus models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Morpheus models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<MorpheusModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Morpheus models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .filter(|model| model.object == "model")
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from Morpheus API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let url = format!("{}/chat/completions", MORPHEUS_API_BASE_URL);

        let request_payload = MorpheusChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(), // Clone messages for the request
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to Morpheus API: model={}, messages_count={}",
               request_payload.model, request_payload.messages.len());

        let response = self.http_client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Morpheus: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Morpheus API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Morpheus API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<MorpheusChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Morpheus response: {}", e) })?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from Morpheus API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("Morpheus API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "Morpheus response was empty or missing choices".to_string() })
        }
    }
}