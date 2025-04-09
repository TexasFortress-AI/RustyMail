use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::dashboard::api::errors::ApiError;

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
    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, ApiError>;
}

// Re-export the provider implementations for easier access
pub use openai::OpenAiAdapter;
pub use openrouter::OpenRouterAdapter; 