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
    email_service: Option<Arc<super::email::EmailService>>,
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
            email_service: None,
            mock_mode: true, // Force mock mode
        }
    }

    pub async fn new(
        openai_api_key: Option<String>,
        openrouter_api_key: Option<String>,
        morpheus_api_key: Option<String>,
        ollama_base_url: Option<String>,
    ) -> Result<Self, String> {
        let mut provider_manager = ProviderManager::new();
        let mut has_real_provider = false;

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
            }).await.ok();
            has_real_provider = true;
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
            }).await.ok();
            has_real_provider = true;
        }

        if let Some(key) = morpheus_api_key {
            provider_manager.add_provider(provider_manager::ProviderConfig {
                name: "morpheus".to_string(),
                provider_type: provider_manager::ProviderType::Morpheus,
                api_key: Some(key),
                model: "llama-3.2-90b-vision-instruct".to_string(), // Will be updated from API
                max_tokens: Some(2000),
                temperature: Some(0.7),
                priority: 3,
                enabled: true,
            }).await.ok();
            has_real_provider = true;
        }

        if let Some(_base_url) = ollama_base_url {
            provider_manager.add_provider(provider_manager::ProviderConfig {
                name: "ollama".to_string(),
                provider_type: provider_manager::ProviderType::Ollama,
                api_key: None, // Ollama doesn't need an API key for local instances
                model: "llama3.2".to_string(), // Will be updated from API
                max_tokens: Some(2000),
                temperature: Some(0.7),
                priority: 4,
                enabled: true,
            }).await.ok();
            has_real_provider = true;
        }

        // Always add mock provider as fallback
        // Priority is lower so real providers are used first when available
        provider_manager.add_provider(provider_manager::ProviderConfig {
            name: "mock".to_string(),
            provider_type: provider_manager::ProviderType::Mock,
            api_key: None,
            model: "mock-model".to_string(),
            max_tokens: Some(2000),
            temperature: Some(0.7),
            priority: if has_real_provider { 99 } else { 1 }, // Lower priority if real providers exist
            enabled: true,
        }).await.ok();

        // Update provider models to use first available model from APIs
        if has_real_provider {
            match provider_manager.update_models_from_api().await {
                Ok(()) => info!("Successfully updated provider models from APIs"),
                Err(e) => warn!("Failed to update some provider models from APIs: {:?}", e),
            }
        }

        // Set the first available provider as current (highest priority = lowest number)
        let providers = provider_manager.list_providers().await;
        if let Some(first_provider) = providers.iter().filter(|p| p.enabled).min_by_key(|p| p.priority) {
            provider_manager.set_current_provider(first_provider.name.clone()).await.ok();
            info!("Set initial current provider to: {}", first_provider.name);
        }

        let nlp_processor = NlpProcessor::new(provider_manager.clone());

        Ok(Self {
            provider_manager,
            nlp_processor,
            conversations: RwLock::new(HashMap::new()),
            email_service: None,
            mock_mode: !has_real_provider, // Set mock mode if no real providers
        })
    }

    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, ApiError> {
        let conversation_id = query.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let query_text = query.query.clone();

        debug!("Processing chatbot query for conversation {}: {}", conversation_id, query_text);

        // Check if this is an email-related query and we have email service
        let email_context = if let Some(email_service) = &self.email_service {
            self.fetch_email_context(&query_text, email_service).await
        } else {
            None
        };

        // DISABLED NLP processor - it's injecting system messages that cause refusals
        // Go directly to the AI provider without NLP interference

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

        // Add a system message that clarifies this is an email assistant
        if messages_history.is_empty() || !messages_history.iter().any(|m| m.role == "system") {
            let mut system_content = "You are RustyMail Assistant, an email management AI. You have full access to the user's email account through the RustyMail system. You can list folders, read emails, search messages, and perform all email operations. Respond naturally to email-related queries.".to_string();

            // Add email context if available
            if let Some(context) = email_context {
                system_content.push_str("\n\nCurrent email data from the user's account:\n");
                system_content.push_str(&context);
            }

            messages_history.insert(0, AiChatMessage {
                role: "system".to_string(),
                content: system_content
            });
        }

        let user_message = AiChatMessage { role: "user".to_string(), content: query_text.clone() };
        messages_history.push(user_message.clone());

        // Get current provider and model info for visibility
        let provider_name = self.provider_manager.get_current_provider_name().await.unwrap_or_else(|| "none".to_string());
        let model_name = self.provider_manager.get_current_model_name().await.unwrap_or_else(|| "none".to_string());

        let response_text = if self.mock_mode {
            warn!("AI Service is in mock mode. Using mock response.");
            format!("[Mock Mode - Provider: mock, Model: mock]\n\n{}", self.generate_mock_response(&query_text))
        } else {
            match self.provider_manager.generate_response(&messages_history).await {
                Ok(text) => {
                    // Prepend provider/model info to response for visibility
                    format!("[Provider: {}, Model: {}]\n\n{}", provider_name, model_name, text)
                },
                Err(e) => {
                    error!("AI Service failed: {}", e);
                    format!("[Error - Provider: {} failed]\n\n{}", provider_name, e.to_string())
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
            "How many emails do I have?".to_string(),
            "List my folders".to_string(),
        ];

        Ok(ChatbotResponse {
            text: response_text,
            conversation_id,
            email_data: None,
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

    // Provider management methods
    pub async fn list_providers(&self) -> Vec<crate::dashboard::services::ai::provider_manager::ProviderConfig> {
        self.provider_manager.list_providers().await
    }

    pub async fn get_current_provider_name(&self) -> Option<String> {
        self.provider_manager.get_current_provider_name().await
    }

    pub async fn set_current_provider(&self, name: String) -> Result<(), String> {
        self.provider_manager.set_current_provider(name)
            .await
            .map_err(|e| format!("Failed to set provider: {}", e))
    }

    pub async fn update_provider_config(&self, name: &str, config: provider_manager::ProviderConfig) -> Result<(), String> {
        self.provider_manager.update_provider_config(name, config)
            .await
            .map_err(|e| format!("Failed to update provider config: {:?}", e))
    }

    pub async fn get_available_models(&self) -> Result<Vec<String>, ApiError> {
        self.provider_manager.get_available_models().await
    }

    /// Set the email service for fetching real emails
    pub fn set_email_service(&mut self, email_service: Arc<super::email::EmailService>) {
        self.email_service = Some(email_service);
    }

    /// Fetch email context based on the query
    async fn fetch_email_context(&self, query: &str, email_service: &Arc<super::email::EmailService>) -> Option<String> {
        let query_lower = query.to_lowercase();

        // Determine what email data to fetch based on the query
        if query_lower.contains("unread") || query_lower.contains("new mail") || query_lower.contains("new email") {
            // Fetch unread emails
            match email_service.get_unread_emails().await {
                Ok(emails) if !emails.is_empty() => {
                    let mut context = format!("Found {} unread emails:\n", emails.len());
                    for (i, email) in emails.iter().take(10).enumerate() {
                        let subject = email.envelope.as_ref()
                            .and_then(|e| e.subject.as_deref())
                            .unwrap_or("No subject");
                        let from = email.envelope.as_ref()
                            .and_then(|e| e.from.first())
                            .map(|addr| format!("{}@{}",
                                addr.mailbox.as_deref().unwrap_or("unknown"),
                                addr.host.as_deref().unwrap_or("unknown")))
                            .unwrap_or_else(|| "Unknown".to_string());
                        let date = email.internal_date
                            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        context.push_str(&format!(
                            "{}. Subject: {}, From: {}, Date: {}\n",
                            i + 1, subject, from, date
                        ));
                    }
                    Some(context)
                },
                Ok(_) => Some("No unread emails found.".to_string()),
                Err(e) => {
                    error!("Failed to fetch unread emails: {}", e);
                    None
                }
            }
        } else if query_lower.contains("inbox") || query_lower.contains("recent") || query_lower.contains("latest") ||
                  query_lower.contains("top") || query_lower.contains("emails") || query_lower.contains("messages") {
            // Fetch recent emails from inbox
            let limit = if query_lower.contains("top 10") || query_lower.contains("10 email") { 10 }
                       else if query_lower.contains("top 5") || query_lower.contains("5 email") { 5 }
                       else if query_lower.contains("top 20") || query_lower.contains("20 email") { 20 }
                       else { 10 };

            match email_service.get_recent_inbox_emails(limit).await {
                Ok(emails) if !emails.is_empty() => {
                    let mut context = format!("Most recent {} emails in inbox:\n", emails.len());
                    for (i, email) in emails.iter().enumerate() {
                        let unread = !email.flags.iter().any(|f| f == "\\Seen" || f == "Seen");
                        let subject = email.envelope.as_ref()
                            .and_then(|e| e.subject.as_deref())
                            .unwrap_or("No subject");
                        let from = email.envelope.as_ref()
                            .and_then(|e| e.from.first())
                            .map(|addr| format!("{}@{}",
                                addr.mailbox.as_deref().unwrap_or("unknown"),
                                addr.host.as_deref().unwrap_or("unknown")))
                            .unwrap_or_else(|| "Unknown".to_string());
                        let date = email.internal_date
                            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        context.push_str(&format!(
                            "{}. Subject: {}, From: {}, Date: {}{}\n",
                            i + 1, subject, from, date,
                            if unread { " [UNREAD]" } else { "" }
                        ));
                    }
                    Some(context)
                },
                Ok(_) => Some("No emails found in inbox.".to_string()),
                Err(e) => {
                    error!("Failed to fetch inbox emails: {}", e);
                    None
                }
            }
        } else if query_lower.contains("folder") || query_lower.contains("mailbox") {
            // List folders
            match email_service.list_folders().await {
                Ok(folders) if !folders.is_empty() => {
                    let mut context = format!("Email folders ({}):\n", folders.len());
                    for folder in folders {
                        context.push_str(&format!("- {}\n", folder));
                    }
                    Some(context)
                },
                Ok(_) => Some("No folders found.".to_string()),
                Err(e) => {
                    error!("Failed to list folders: {}", e);
                    None
                }
            }
        } else {
            None
        }
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
