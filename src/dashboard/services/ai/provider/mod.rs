// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use crate::api::errors::ApiError as RestApiError;

pub mod openai;
pub mod openrouter;
pub mod morpheus;
pub mod ollama;
pub mod anthropic;
pub mod deepseek;
pub mod xai;
pub mod gemini;
pub mod mistral;
pub mod together;
pub mod azure;

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

    /// Fetches the list of available models from the provider's API.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of model names (`Vec<String>`) or an `ApiError`.
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError>;
}

// Re-export the provider implementations for easier access
pub use openai::OpenAiAdapter;
pub use openrouter::OpenRouterAdapter;
pub use morpheus::MorpheusAdapter;
pub use ollama::OllamaAdapter;
pub use anthropic::AnthropicAdapter;
pub use deepseek::DeepSeekAdapter;
pub use xai::XAIAdapter;
pub use gemini::GeminiAdapter;
pub use mistral::MistralAdapter;
pub use together::TogetherAdapter;
pub use azure::AzureOpenAIAdapter;

// --- Mock Provider Implementation ---
#[derive(Debug, Default)]
pub struct MockAiProvider;

#[async_trait]
impl AiProvider for MockAiProvider {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        Ok(vec![
            "mock-gpt-4".to_string(),
            "mock-claude-3.5".to_string(),
            "mock-llama-2".to_string(),
        ])
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        if let Some(last_message) = messages.last() {
            if last_message.role == "user" {
                let content = last_message.content.to_lowercase();

                // Provide contextual mock responses for email-related queries
                if content.contains("how many") && content.contains("email") {
                    return Ok("You have 42 emails in your inbox. 15 are unread, including 3 marked as important.".to_string());
                } else if content.contains("unread") || content.contains("new") {
                    return Ok("You have 15 unread emails. The most recent is from 'John Doe' with the subject 'Q4 Revenue Report'.".to_string());
                } else if content.contains("latest") || content.contains("recent") {
                    return Ok("Your most recent email is from 'Sarah Smith' received 10 minutes ago about 'Project Update - Sprint 23'.".to_string());
                } else if content.contains("important") || content.contains("priority") {
                    return Ok("You have 3 important emails:\n1. Budget Approval Request - Finance Team\n2. Client Meeting Tomorrow - Sales Team\n3. Security Update Required - IT Department".to_string());
                } else if content.contains("spam") || content.contains("junk") {
                    return Ok("Your spam folder contains 127 emails. I've identified 5 legitimate emails that may have been incorrectly filtered.".to_string());
                } else if content.contains("search") || content.contains("find") {
                    return Ok("I found 8 emails matching your criteria. The most relevant one is from 'Mike Johnson' about 'Meeting Notes - Product Roadmap'.".to_string());
                } else if content.contains("compose") || content.contains("write") {
                    return Ok("I can help you compose an email. What would you like to write about?".to_string());
                } else if content.contains("folder") || content.contains("label") {
                    return Ok("You have the following folders: Inbox (42), Sent (156), Drafts (3), Archive (1,234), Spam (127), and Trash (89).".to_string());
                } else if content.contains("attachment") {
                    return Ok("You have 12 emails with attachments. The largest attachment (15MB) is in an email from 'Design Team' about 'Brand Guidelines v2.3'.".to_string());
                } else if content.contains("calendar") || content.contains("meeting") {
                    return Ok("You have 3 meeting invitations in your inbox:\n1. Team Standup - Tomorrow 9:00 AM\n2. Client Review - Thursday 2:00 PM\n3. All Hands - Friday 4:00 PM".to_string());
                } else if content.contains("hello") || content.contains("hi") {
                    return Ok("Hello! I'm your email assistant. I can help you manage your inbox, search for emails, compose messages, and organize your folders. How can I assist you today?".to_string());
                } else if content.contains("help") {
                    return Ok("I can help you with:\n• Checking email counts and unread messages\n• Searching for specific emails\n• Managing folders and labels\n• Composing new emails\n• Finding attachments\n• Reviewing meeting invitations\nWhat would you like to do?".to_string());
                } else {
                    // Generic fallback response
                    return Ok(format!("I understand you're asking about '{}'. In a production environment, I would connect to your email server to provide accurate information. For now, I'm returning mock data for testing purposes.", last_message.content));
                }
            }
        }
        Ok("I'm your email assistant. Ask me about your emails, and I'll help you manage your inbox!".to_string())
    }
}

// --- Comment out mock module as it doesn't exist --- 
// mod mock; 
// pub use mock::MockAiProvider; 