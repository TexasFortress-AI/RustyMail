use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use crate::api::errors::ApiError as RestApiError;

pub mod openai;
pub mod openrouter;

/// Common message structure for AI chat completion APIs (OpenAI, OpenRouter)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiChatMessage {
    pub role: String,
    pub content: String,
}

/// Trait defining the interface for an AI chat completion provider.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Generates a chat completion response based on the provided message history.
    ///
    /// # Arguments
    ///
    /// * `messages` - A slice of `AiChatMessage` representing the conversation history.
    ///
    /// # Returns
    ///
    /// A `Result` containing the AI's response text (`String`) or an `ApiError`.
    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError>;
}

// Re-export the provider implementations for easier access
pub use openai::OpenAiAdapter;
pub use openrouter::OpenRouterAdapter;

// --- Mock Provider Implementation ---
#[derive(Debug, Default)]
pub struct MockAiProvider;

#[async_trait]
impl AiProvider for MockAiProvider {
    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        if let Some(last_message) = messages.last() {
            if last_message.role == "user" {
                return Ok(format!("Mock response to: {}", last_message.content));
            }
        }
        Ok("This is a mock AI response.".to_string())
    }
}

// --- Comment out mock module as it doesn't exist --- 
// mod mock; 
// pub use mock::MockAiProvider; 