// src/dashboard/services/ai/providers/openrouter.rs

use async_trait::async_trait;
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT}};
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::api::errors::ApiError as RestApiError;
use super::{AiProvider, AiChatMessage}; // Import trait and common message struct

// OpenRouter API constants
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const DEFAULT_OPENROUTER_MODEL: &str = "deepseek/deepseek-coder-v2-lite-instruct";
const REFERER_HEADER_VALUE: &str = "http://localhost/rustymail-dashboard"; // Example value
const TITLE_HEADER_VALUE: &str = "RustyMail Dashboard"; // Example value

// --- OpenRouter Specific Request/Response Structs ---
// OpenRouter uses OpenAI-compatible request/response structure for basic chat completions
// So we can reuse the request/response structs if they were defined commonly,
// or redefine them here if they are specific to OpenRouter's nuances.
// For now, let's assume standard OpenAI compatibility is sufficient.

#[derive(Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<AiChatMessage>,
    // OpenRouter might support additional parameters, add them here if needed
}

// Assuming standard OpenAI response format
#[derive(Deserialize)]
struct OpenRouterChatResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: AiChatMessage,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModel {
    id: String,
}
// --- End Structs ---

#[derive(Debug)]
pub struct OpenRouterAdapter {
    api_key: String,
    http_client: Client,
    model: String,
    // Store common headers
    common_headers: HeaderMap,
}

impl OpenRouterAdapter {
    pub fn new(api_key: String, http_client: Client) -> Self {
        let mut common_headers = HeaderMap::new();
        // Required headers for OpenRouter
        common_headers.insert("HTTP-Referer", HeaderValue::from_static(REFERER_HEADER_VALUE));
        common_headers.insert("X-Title", HeaderValue::from_static(TITLE_HEADER_VALUE));
        // Add a user agent
        common_headers.insert(USER_AGENT, HeaderValue::from_static("RustyMail/Dashboard"));

        Self {
            api_key,
            http_client,
            model: DEFAULT_OPENROUTER_MODEL.to_string(),
            common_headers,
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
impl AiProvider for OpenRouterAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        debug!("Fetching available models from OpenRouter API");

        let response = self.http_client
            .get(OPENROUTER_MODELS_URL)
            .headers(self.common_headers.clone())
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("OpenRouter models: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenRouter models API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("OpenRouter models API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OpenRouterModelsResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize OpenRouter models response: {}", e) })?;

        let models: Vec<String> = response_body.data
            .into_iter()
            .map(|model| model.id)
            .collect();

        debug!("Retrieved {} models from OpenRouter API", models.len());
        Ok(models)
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let request_payload = OpenRouterChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
        };

        debug!("Sending request to OpenRouter API: model={}, messages_count={}", 
               request_payload.model, request_payload.messages.len());

        let response = self.http_client
            .post(OPENROUTER_API_URL)
            // Set common headers (Referer, X-Title, User-Agent)
            .headers(self.common_headers.clone()) 
            // Set authorization header
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(60)) // Slightly longer timeout maybe?
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("OpenRouter: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenRouter API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("OpenRouter API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<OpenRouterChatResponse>() // Use OpenRouter specific response struct
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize OpenRouter response: {}", e) })?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from OpenRouter API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("OpenRouter API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "OpenRouter response was empty or missing choices".to_string() })
        }
    }
} 