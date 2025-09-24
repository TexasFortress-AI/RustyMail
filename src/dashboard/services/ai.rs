pub mod provider;
pub mod provider_manager;
pub mod nlp_processor;

use log::{debug, error, info, warn};
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData};
use crate::dashboard::services::ai::provider::{AiProvider, AiChatMessage};
use crate::dashboard::services::ai::provider_manager::ProviderManager;
use crate::dashboard::services::ai::nlp_processor::NlpProcessor;
use std::sync::Arc;
use crate::api::errors::ApiError;
use thiserror::Error;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use reqwest::Client;

// Conversation history entry
#[derive(Debug, Clone)]
struct ConversationEntry {
    message: AiChatMessage, // Use the common message struct
    timestamp: chrono::DateTime<chrono::Utc>,
}

// Conversation history
#[derive(Debug, Clone, Default)]
struct Conversation {
    entries: Vec<ConversationEntry>,
    last_activity: chrono::DateTime<chrono::Utc>,
}

pub struct AiService {
    conversations: RwLock<HashMap<String, Conversation>>,
    provider_manager: ProviderManager,
    nlp_processor: NlpProcessor,
    mock_mode: bool, // Flag to force mock responses
}

impl std::fmt::Debug for AiService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiService")
            .field("conversations_count", &self.conversations.try_read().map(|g| g.len()).unwrap_or(0))
            .field("mock_mode", &self.mock_mode)
            .finish()
    }
}

// Define AI Service Error
#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider error: {0}")]
    ProviderError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("API Error during AI operation: {0}")]
    ApiError(#[from] crate::api::errors::ApiError),
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),
}

impl AiService {
    /// Creates a new mock AiService instance for testing
    pub fn new_mock() -> Self {
        let provider_manager = ProviderManager::new();
        let nlp_processor = NlpProcessor::new(provider_manager.clone());

        Self {
            provider_manager,
            nlp_processor,
            conversations: RwLock::new(HashMap::new()),
            mock_mode: true, // Force mock mode
        }
    }

    pub fn new(
        openai_api_key: Option<String>,
        openrouter_api_key: Option<String>,
    ) -> Result<Self, String> {
        let provider_manager = ProviderManager::new();

        // Configure providers
        if let Some(key) = openai_api_key {
            provider_manager.add_provider(provider_manager::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_manager::ProviderType::OpenAI,
                api_key: Some(key),
                model: "gpt-3.5-turbo".to_string(),
                max_tokens: Some(2000),
                temperature: Some(0.7),
                priority: 1,
                enabled: true,
            }).ok();
        }

        if let Some(key) = openrouter_api_key {
            provider_manager.add_provider(provider_manager::ProviderConfig {
                name: "openrouter".to_string(),
                provider_type: provider_manager::ProviderType::OpenRouter,
                api_key: Some(key),
                model: "meta-llama/llama-2-70b-chat".to_string(),
                max_tokens: Some(2000),
                temperature: Some(0.7),
                priority: 2,
                enabled: true,
            }).ok();
        }

        let nlp_processor = NlpProcessor::new(provider_manager.clone());

        Ok(Self {
            provider_manager,
            nlp_processor,
            conversations: RwLock::new(HashMap::new()),
            mock_mode: false,
        })
    }

    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, ApiError> {
        let conversation_id = query.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let query_text = query.query.clone();

        debug!("Processing chatbot query for conversation {}: {}", conversation_id, query_text);

        // First, try to process as an email query using NLP
        let nlp_result = self.nlp_processor.process_query(
            &query_text,
            None // TODO: Add conversation context support
        ).await;

        let (response_text, email_data, suggestions) = match nlp_result {
            Ok(nlp_res) if nlp_res.confidence > 0.6 => {
                // High confidence NLP result - execute MCP operation
                info!("NLP detected intent: {:?} with confidence {}", nlp_res.intent, nlp_res.confidence);

                let response = if let Some(mcp_op) = &nlp_res.mcp_operation {
                    format!(
                        "I understood you want to {}. Executing: {} with parameters: {}",
                        nlp_res.intent.to_string(),
                        mcp_op.method,
                        mcp_op.params
                    )
                } else {
                    format!("I understood you want to {}, but I couldn't map it to a specific operation.", nlp_res.intent.to_string())
                };

                let suggestions = vec![
                    "Show me my folders".to_string(),
                    "List unread emails".to_string(),
                    "Search for emails from john@example.com".to_string(),
                ];

                (response, None, suggestions)
            },
            _ => {
                // Low confidence or error - use regular AI chat
                let mut conversations = self.conversations.write().await;
                let conversation = conversations
                    .entry(conversation_id.clone())
                    .or_insert_with(|| {
                        debug!("Creating new conversation: {}", conversation_id);
                        Conversation {
                            entries: Vec::new(),
                            last_activity: chrono::Utc::now(),
                        }
                    });

                conversation.last_activity = chrono::Utc::now();

                let mut messages_history: Vec<AiChatMessage> = conversation.entries.iter()
                    .map(|entry| entry.message.clone())
                    .collect();

                let user_message = AiChatMessage { role: "user".to_string(), content: query_text.clone() };
                messages_history.push(user_message.clone());

                let response_text = if self.mock_mode {
                    warn!("AI Service is in mock mode. Using mock response.");
                    self.generate_mock_response(&query_text)
                } else {
                    match self.provider_manager.generate_response(&messages_history).await {
                        Ok(text) => text,
                        Err(e) => {
                            error!("AI Service failed to get response: {}. Falling back to mock.", e);
                            self.generate_mock_response(&query_text)
                        }
                    }
                };

                let assistant_message = AiChatMessage { role: "assistant".to_string(), content: response_text.clone() };
                conversation.entries.push(ConversationEntry {
                    message: user_message,
                    timestamp: chrono::Utc::now(),
                });
                conversation.entries.push(ConversationEntry {
                    message: assistant_message,
                    timestamp: chrono::Utc::now(),
                });

                let suggestions = vec![
                    "Show me my unread emails".to_string(),
                    "How many emails do I have from support?".to_string(),
                    "What's in my Sent folder?".to_string(),
                ];

                (response_text, None, suggestions)
            }
        };

        Ok(ChatbotResponse {
            text: response_text,
            conversation_id,
            email_data,
            followup_suggestions: Some(suggestions),
        })
    }

    // Generate a mock response for testing or fallback
    fn generate_mock_response(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        
        if query_lower.contains("hello") || query_lower.contains("hi") {
            "Hello! I'm the RustyMail assistant. How can I help you with your emails today? (Mock Response)".to_string()
        } else if query_lower.contains("unread") {
            "You have 3 unread emails in your inbox. Would you like me to show them to you? (Mock Response)".to_string()
        } else if query_lower.contains("inbox") {
            "Your inbox contains 24 messages total, with 3 unread. (Mock Response)".to_string()
        } else if query_lower.contains("sent") {
            "Your Sent folder contains 12 messages. (Mock Response)".to_string()
        } else {
            "I'm currently configured to provide mock responses. Please provide an OpenAI API key for full functionality.".to_string()
        }
    }
    
    // Generate mock email data for testing
    #[allow(dead_code)]
    fn generate_mock_email_data(&self) -> EmailData {
        EmailData { messages: None, count: None, folders: None } // Simplified for example
    }
    
    // Clean up old conversations
    #[allow(dead_code)]
    async fn cleanup_old_conversations(&self, conversations: &mut HashMap<String, Conversation>) {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();
        
        // Find conversations older than 24 hours
        for (id, convo) in conversations.iter() {
            if (now - convo.last_activity).num_hours() > 24 {
                to_remove.push(id.clone());
            }
        }
        
        // Remove old conversations
        for id in to_remove {
            conversations.remove(&id);
            debug!("Removed old conversation: {}", id);
        }
    }

}
