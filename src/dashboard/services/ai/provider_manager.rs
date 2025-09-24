// src/dashboard/services/ai/provider_manager.rs
// Unified AI Provider Management with dynamic selection and failover

use async_trait::async_trait;
use log::{debug, warn, error, info};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::api::errors::ApiError as RestApiError;
use super::provider::{AiProvider, AiChatMessage, OpenAiAdapter, OpenRouterAdapter, MockAiProvider};
use reqwest::Client;

// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub priority: u32,  // Lower number = higher priority
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderType {
    OpenAI,
    OpenRouter,
    Mock,
}

// Provider manager for handling multiple AI providers
#[derive(Clone)]
pub struct ProviderManager {
    providers: Arc<RwLock<HashMap<String, Arc<dyn AiProvider>>>>,
    configs: Arc<RwLock<Vec<ProviderConfig>>>,
    current_provider: Arc<RwLock<Option<String>>>,
    http_client: Client,
}

impl ProviderManager {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(Vec::new())),
            current_provider: Arc::new(RwLock::new(None)),
            http_client: Client::new(),
        }
    }

    pub fn add_provider(&self, config: ProviderConfig) -> Result<(), RestApiError> {
        let provider: Arc<dyn AiProvider> = match config.provider_type {
            ProviderType::OpenAI => {
                Arc::new(OpenAiAdapter::new(
                    config.api_key.clone().ok_or_else(|| RestApiError::InternalError {
                        message: "OpenAI API key is required".to_string()
                    })?,
                    self.http_client.clone()
                ))
            },
            ProviderType::OpenRouter => {
                Arc::new(OpenRouterAdapter::new(
                    config.api_key.clone().ok_or_else(|| RestApiError::InternalError {
                        message: "OpenRouter API key is required".to_string()
                    })?,
                    self.http_client.clone()
                ))
            },
            ProviderType::Mock => Arc::new(MockAiProvider),
        };

        self.providers.write().unwrap().insert(config.name.clone(), provider);
        self.configs.write().unwrap().push(config);
        Ok(())
    }

    // Initialize from environment variables
    pub async fn init_from_env(&mut self) -> Result<(), RestApiError> {
        let mut configs = Vec::new();

        // Check for OpenAI configuration
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let config = ProviderConfig {
                name: "openai".to_string(),
                provider_type: ProviderType::OpenAI,
                api_key: Some(api_key.clone()),
                model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string()),
                max_tokens: std::env::var("OPENAI_MAX_TOKENS")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                temperature: std::env::var("OPENAI_TEMPERATURE")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                priority: 1,
                enabled: true,
            };
            configs.push(config);

            // Create OpenAI provider
            let provider = Arc::new(OpenAiAdapter::new(api_key, self.http_client.clone()));
            self.providers.insert("openai".to_string(), provider);
            info!("Initialized OpenAI provider");
        }

        // Check for OpenRouter configuration
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            let config = ProviderConfig {
                name: "openrouter".to_string(),
                provider_type: ProviderType::OpenRouter,
                api_key: Some(api_key.clone()),
                model: std::env::var("OPENROUTER_MODEL")
                    .unwrap_or_else(|_| "deepseek/deepseek-coder-v2-lite-instruct".to_string()),
                max_tokens: std::env::var("OPENROUTER_MAX_TOKENS")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                temperature: std::env::var("OPENROUTER_TEMPERATURE")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                priority: 2,
                enabled: true,
            };
            configs.push(config);

            // Create OpenRouter provider
            let provider = Arc::new(OpenRouterAdapter::new(api_key, self.http_client.clone()));
            self.providers.insert("openrouter".to_string(), provider);
            info!("Initialized OpenRouter provider");
        }

        // Always add mock provider as fallback
        let mock_config = ProviderConfig {
            name: "mock".to_string(),
            provider_type: ProviderType::Mock,
            api_key: None,
            model: "mock-model".to_string(),
            max_tokens: None,
            temperature: None,
            priority: 99,
            enabled: true,
        };
        configs.push(mock_config);
        self.providers.insert("mock".to_string(), Arc::new(MockAiProvider));
        info!("Initialized Mock provider as fallback");

        // Sort configs by priority
        configs.sort_by_key(|c| c.priority);

        // Set the first enabled provider as current
        if let Some(first_enabled) = configs.iter().find(|c| c.enabled) {
            *self.current_provider.write().await = Some(first_enabled.name.clone());
            info!("Set current provider to: {}", first_enabled.name);
        }

        *self.configs.write().await = configs;

        Ok(())
    }

    // Add a provider with configuration
    pub async fn add_provider(&mut self, config: ProviderConfig) -> Result<(), RestApiError> {
        let provider: Arc<dyn AiProvider> = match config.provider_type {
            ProviderType::OpenAI => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "OpenAI provider requires API key".to_string()
                    })?;
                Arc::new(OpenAiAdapter::new(api_key.clone(), self.http_client.clone()))
            },
            ProviderType::OpenRouter => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "OpenRouter provider requires API key".to_string()
                    })?;
                Arc::new(OpenRouterAdapter::new(api_key.clone(), self.http_client.clone()))
            },
            ProviderType::Mock => {
                Arc::new(MockAiProvider)
            },
        };

        self.providers.insert(config.name.clone(), provider);

        let mut configs = self.configs.write().await;
        configs.push(config.clone());
        configs.sort_by_key(|c| c.priority);

        info!("Added provider: {}", config.name);
        Ok(())
    }

    // Get current provider
    pub async fn get_current_provider(&self) -> Option<Arc<dyn AiProvider>> {
        let current = self.current_provider.read().await;
        if let Some(name) = current.as_ref() {
            self.providers.get(name).cloned()
        } else {
            None
        }
    }

    // Set current provider
    pub async fn set_current_provider(&self, name: String) -> Result<(), RestApiError> {
        if !self.providers.contains_key(&name) {
            return Err(RestApiError::UnprocessableEntity {
                message: format!("Provider '{}' not found", name)
            });
        }

        *self.current_provider.write().await = Some(name.clone());
        info!("Switched current provider to: {}", name);
        Ok(())
    }

    // Generate response with automatic failover
    pub async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let configs = self.configs.read().await;

        for config in configs.iter().filter(|c| c.enabled) {
            if let Some(provider) = self.providers.get(&config.name) {
                debug!("Trying provider: {}", config.name);

                match provider.generate_response(messages).await {
                    Ok(response) => {
                        debug!("Successfully got response from provider: {}", config.name);
                        return Ok(response);
                    },
                    Err(e) => {
                        warn!("Provider {} failed: {}. Trying next provider...", config.name, e);
                        continue;
                    }
                }
            }
        }

        error!("All providers failed");
        Err(RestApiError::ServiceUnavailable {
            service: "No AI providers available".to_string()
        })
    }

    // List available providers
    pub async fn list_providers(&self) -> Vec<ProviderConfig> {
        self.configs.read().await.clone()
    }

    // Enable/disable a provider
    pub async fn set_provider_enabled(&self, name: &str, enabled: bool) -> Result<(), RestApiError> {
        let mut configs = self.configs.write().await;

        if let Some(config) = configs.iter_mut().find(|c| c.name == name) {
            config.enabled = enabled;
            info!("Provider '{}' enabled: {}", name, enabled);
            Ok(())
        } else {
            Err(RestApiError::UnprocessableEntity {
                message: format!("Provider '{}' not found", name)
            })
        }
    }

    // Update provider configuration
    pub async fn update_provider_config(&mut self, name: &str, config: ProviderConfig) -> Result<(), RestApiError> {
        // Remove old provider
        self.providers.remove(name);

        // Add new provider with updated config
        self.add_provider(config).await?;

        Ok(())
    }
}

// Conversation context management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub id: String,
    pub messages: Vec<AiChatMessage>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub max_messages: usize,
}

impl ConversationContext {
    pub fn new(id: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            messages: Vec::new(),
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            max_messages: 50,  // Default max messages to keep in context
        }
    }

    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(AiChatMessage { role, content });
        self.updated_at = chrono::Utc::now();

        // Automatically truncate if exceeding max messages
        if self.messages.len() > self.max_messages {
            let start = self.messages.len() - self.max_messages;
            self.messages = self.messages[start..].to_vec();
        }
    }

    pub fn get_messages(&self) -> &[AiChatMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = chrono::Utc::now();
    }

    pub fn set_max_messages(&mut self, max: usize) {
        self.max_messages = max;
        if self.messages.len() > max {
            let start = self.messages.len() - max;
            self.messages = self.messages[start..].to_vec();
        }
    }
}

// Context manager for handling multiple conversations
pub struct ConversationManager {
    contexts: Arc<RwLock<HashMap<String, ConversationContext>>>,
    max_conversations: usize,
}

impl ConversationManager {
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            max_conversations: 100,  // Maximum number of conversations to keep
        }
    }

    pub async fn get_or_create(&self, id: String) -> ConversationContext {
        let mut contexts = self.contexts.write().await;

        // Clean up old conversations if at max capacity
        if contexts.len() >= self.max_conversations && !contexts.contains_key(&id) {
            self.cleanup_old_conversations(&mut contexts).await;
        }

        contexts.entry(id.clone())
            .or_insert_with(|| ConversationContext::new(id.clone()))
            .clone()
    }

    pub async fn update(&self, id: String, context: ConversationContext) {
        let mut contexts = self.contexts.write().await;
        contexts.insert(id, context);
    }

    pub async fn remove(&self, id: &str) -> Option<ConversationContext> {
        let mut contexts = self.contexts.write().await;
        contexts.remove(id)
    }

    pub async fn clear_all(&self) {
        let mut contexts = self.contexts.write().await;
        contexts.clear();
    }

    pub async fn list_conversations(&self) -> Vec<String> {
        let contexts = self.contexts.read().await;
        contexts.keys().cloned().collect()
    }

    // Clean up old conversations (keep most recent ones)
    async fn cleanup_old_conversations(&self, contexts: &mut HashMap<String, ConversationContext>) {
        let mut conversations: Vec<_> = contexts.iter()
            .map(|(id, ctx)| (id.clone(), ctx.updated_at))
            .collect();

        // Sort by updated_at (oldest first)
        conversations.sort_by_key(|(_, time)| *time);

        // Remove oldest conversations to make room
        let to_remove = conversations.len() / 4;  // Remove 25% of oldest conversations
        for (id, _) in conversations.iter().take(to_remove) {
            contexts.remove(id);
            debug!("Removed old conversation: {}", id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conversation_context() {
        let mut context = ConversationContext::new("test".to_string());

        context.add_message("user".to_string(), "Hello".to_string());
        context.add_message("assistant".to_string(), "Hi there!".to_string());

        assert_eq!(context.messages.len(), 2);
        assert_eq!(context.messages[0].role, "user");
        assert_eq!(context.messages[1].role, "assistant");

        context.set_max_messages(1);
        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.messages[0].role, "assistant");
    }

    #[tokio::test]
    async fn test_conversation_manager() {
        let manager = ConversationManager::new();

        let mut context = manager.get_or_create("session1".to_string()).await;
        context.add_message("user".to_string(), "Test message".to_string());

        manager.update("session1".to_string(), context).await;

        let retrieved = manager.get_or_create("session1".to_string()).await;
        assert_eq!(retrieved.messages.len(), 1);

        manager.remove("session1").await;
        let new_context = manager.get_or_create("session1".to_string()).await;
        assert_eq!(new_context.messages.len(), 0);
    }

    #[tokio::test]
    async fn test_provider_manager_init() {
        let mut manager = ProviderManager::new();

        // This should always succeed as it adds mock provider
        manager.init_from_env().await.unwrap();

        let providers = manager.list_providers().await;
        assert!(!providers.is_empty());

        // Should have at least mock provider
        assert!(providers.iter().any(|p| p.provider_type == ProviderType::Mock));
    }
}