// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/provider_manager.rs
// Unified AI Provider Management with dynamic selection and failover

use async_trait::async_trait;
use log::{debug, warn, error, info};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::api::errors::ApiError as RestApiError;
use super::provider::{
    AiProvider, AiChatMessage,
    OpenAiAdapter, OpenRouterAdapter, MorpheusAdapter, OllamaAdapter, MockAiProvider,
    AnthropicAdapter, DeepSeekAdapter, XAIAdapter, GeminiAdapter,
    MistralAdapter, TogetherAdapter, AzureOpenAIAdapter
};
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
    Morpheus,
    Ollama,
    Anthropic,
    DeepSeek,
    XAI,
    Gemini,
    Mistral,
    Together,
    Azure,
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


    // Initialize from environment variables
    pub async fn init_from_env(&mut self) -> Result<(), RestApiError> {
        let mut configs = Vec::new();

        // Check for OpenAI configuration
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            if let Ok(model) = std::env::var("OPENAI_MODEL") {
                let config = ProviderConfig {
                    name: "openai".to_string(),
                    provider_type: ProviderType::OpenAI,
                    api_key: Some(api_key.clone()),
                    model,
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
                self.providers.write().await.insert("openai".to_string(), provider);
                info!("Initialized OpenAI provider");
            } else {
                warn!("OPENAI_API_KEY is set but OPENAI_MODEL is not - skipping OpenAI provider");
            }
        }

        // Check for OpenRouter configuration
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            if let Ok(model) = std::env::var("OPENROUTER_MODEL") {
                let config = ProviderConfig {
                    name: "openrouter".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    api_key: Some(api_key.clone()),
                    model,
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
                self.providers.write().await.insert("openrouter".to_string(), provider);
                info!("Initialized OpenRouter provider");
            } else {
                warn!("OPENROUTER_API_KEY is set but OPENROUTER_MODEL is not - skipping OpenRouter provider");
            }
        }

        // Check for Morpheus configuration
        if let Ok(api_key) = std::env::var("MORPHEUS_API_KEY") {
            if let Ok(model) = std::env::var("MORPHEUS_MODEL") {
                let config = ProviderConfig {
                    name: "morpheus".to_string(),
                    provider_type: ProviderType::Morpheus,
                    api_key: Some(api_key.clone()),
                    model: model.clone(),
                    max_tokens: std::env::var("MORPHEUS_MAX_TOKENS")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                    temperature: std::env::var("MORPHEUS_TEMPERATURE")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                    priority: 3,
                    enabled: true,
                };
                configs.push(config.clone());

                // Create Morpheus provider
                let provider = Arc::new(MorpheusAdapter::new(api_key, self.http_client.clone())
                    .with_model(model));
                self.providers.write().await.insert("morpheus".to_string(), provider);
                info!("Initialized Morpheus provider");
            } else {
                warn!("MORPHEUS_API_KEY is set but MORPHEUS_MODEL is not - skipping Morpheus provider");
            }
        }

        // Check for Ollama configuration
        if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
            if let Ok(model) = std::env::var("OLLAMA_MODEL") {
                let config = ProviderConfig {
                    name: "ollama".to_string(),
                    provider_type: ProviderType::Ollama,
                    api_key: None, // Ollama doesn't require an API key for local instances
                    model,
                    max_tokens: std::env::var("OLLAMA_MAX_TOKENS")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                    temperature: std::env::var("OLLAMA_TEMPERATURE")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                    priority: 4,
                    enabled: true,
                };
                configs.push(config);

                // Create Ollama provider
                let provider = Arc::new(OllamaAdapter::new(base_url, self.http_client.clone()));
                self.providers.write().await.insert("ollama".to_string(), provider);
                info!("Initialized Ollama provider");
            } else {
                warn!("OLLAMA_BASE_URL is set but OLLAMA_MODEL is not - skipping Ollama provider");
            }
        }

        // Check for Anthropic Claude configuration
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
                let config = ProviderConfig {
                    name: "anthropic".to_string(),
                    provider_type: ProviderType::Anthropic,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 5,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(AnthropicAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("anthropic".to_string(), provider);
                info!("Initialized Anthropic Claude provider");
            } else {
                warn!("ANTHROPIC_API_KEY is set but ANTHROPIC_MODEL is not - skipping Anthropic provider");
            }
        }

        // Check for DeepSeek configuration
        if let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY") {
            if let Ok(model) = std::env::var("DEEPSEEK_MODEL") {
                let config = ProviderConfig {
                    name: "deepseek".to_string(),
                    provider_type: ProviderType::DeepSeek,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 6,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(DeepSeekAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("deepseek".to_string(), provider);
                info!("Initialized DeepSeek provider");
            } else {
                warn!("DEEPSEEK_API_KEY is set but DEEPSEEK_MODEL is not - skipping DeepSeek provider");
            }
        }

        // Check for xAI (Grok) configuration
        if let Ok(api_key) = std::env::var("XAI_API_KEY") {
            if let Ok(model) = std::env::var("XAI_MODEL") {
                let config = ProviderConfig {
                    name: "xai".to_string(),
                    provider_type: ProviderType::XAI,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 7,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(XAIAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("xai".to_string(), provider);
                info!("Initialized xAI (Grok) provider");
            } else {
                warn!("XAI_API_KEY is set but XAI_MODEL is not - skipping xAI provider");
            }
        }

        // Check for Google Gemini configuration
        if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            if let Ok(model) = std::env::var("GEMINI_MODEL") {
                let config = ProviderConfig {
                    name: "gemini".to_string(),
                    provider_type: ProviderType::Gemini,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 8,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(GeminiAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("gemini".to_string(), provider);
                info!("Initialized Google Gemini provider");
            } else {
                warn!("GEMINI_API_KEY is set but GEMINI_MODEL is not - skipping Gemini provider");
            }
        }

        // Check for Mistral AI configuration
        if let Ok(api_key) = std::env::var("MISTRAL_API_KEY") {
            if let Ok(model) = std::env::var("MISTRAL_MODEL") {
                let config = ProviderConfig {
                    name: "mistral".to_string(),
                    provider_type: ProviderType::Mistral,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 9,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(MistralAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("mistral".to_string(), provider);
                info!("Initialized Mistral AI provider");
            } else {
                warn!("MISTRAL_API_KEY is set but MISTRAL_MODEL is not - skipping Mistral provider");
            }
        }

        // Check for Together AI configuration
        if let Ok(api_key) = std::env::var("TOGETHER_API_KEY") {
            if let Ok(model) = std::env::var("TOGETHER_MODEL") {
                let config = ProviderConfig {
                    name: "together".to_string(),
                    provider_type: ProviderType::Together,
                    api_key: Some(api_key.clone()),
                    model,
                    max_tokens: None,
                    temperature: None,
                    priority: 10,
                    enabled: true,
                };
                configs.push(config);
                let provider = Arc::new(TogetherAdapter::new(api_key, self.http_client.clone()));
                self.providers.write().await.insert("together".to_string(), provider);
                info!("Initialized Together AI provider");
            } else {
                warn!("TOGETHER_API_KEY is set but TOGETHER_MODEL is not - skipping Together provider");
            }
        }

        // Check for Azure OpenAI configuration
        if let Ok(api_key) = std::env::var("AZURE_OPENAI_API_KEY") {
            // Azure requires endpoint to be set
            if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
                let config = ProviderConfig {
                    name: "azure".to_string(),
                    provider_type: ProviderType::Azure,
                    api_key: Some(api_key.clone()),
                    model: std::env::var("AZURE_OPENAI_DEPLOYMENT")
                        .expect("AZURE_OPENAI_DEPLOYMENT environment variable must be set when using Azure OpenAI provider"),
                    max_tokens: None,
                    temperature: None,
                    priority: 11,
                    enabled: true,
                };
                configs.push(config);
                match AzureOpenAIAdapter::new(api_key, self.http_client.clone()) {
                    Ok(provider) => {
                        self.providers.write().await.insert("azure".to_string(), Arc::new(provider));
                        info!("Initialized Azure OpenAI provider");
                    }
                    Err(e) => {
                        warn!("Failed to initialize Azure OpenAI provider: {:?}", e);
                    }
                }
            }
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
        self.providers.write().await.insert("mock".to_string(), Arc::new(MockAiProvider));
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
                Arc::new(OpenRouterAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Morpheus => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Morpheus provider requires API key".to_string()
                    })?;
                Arc::new(MorpheusAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Ollama => {
                // For Ollama, we need a base URL from environment variable
                let base_url = std::env::var("OLLAMA_BASE_URL")
                    .map_err(|_| RestApiError::UnprocessableEntity {
                        message: "Ollama provider requires OLLAMA_BASE_URL environment variable to be set".to_string()
                    })?;
                Arc::new(OllamaAdapter::new(base_url, self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Anthropic => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Anthropic provider requires API key".to_string()
                    })?;
                Arc::new(AnthropicAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::DeepSeek => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "DeepSeek provider requires API key".to_string()
                    })?;
                Arc::new(DeepSeekAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::XAI => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "xAI provider requires API key".to_string()
                    })?;
                Arc::new(XAIAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Gemini => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Gemini provider requires API key".to_string()
                    })?;
                Arc::new(GeminiAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Mistral => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Mistral provider requires API key".to_string()
                    })?;
                Arc::new(MistralAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Together => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Together AI provider requires API key".to_string()
                    })?;
                Arc::new(TogetherAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Azure => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Azure OpenAI provider requires API key".to_string()
                    })?;
                AzureOpenAIAdapter::new(api_key.clone(), self.http_client.clone())
                    .map(|adapter| Arc::new(adapter) as Arc<dyn AiProvider>)?
            },
            ProviderType::Mock => {
                Arc::new(MockAiProvider)
            },
        };

        self.providers.write().await.insert(config.name.clone(), provider);

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
            let providers = self.providers.read().await;
            providers.get(name).cloned()
        } else {
            None
        }
    }

    // Set current provider
    pub async fn set_current_provider(&self, name: String) -> Result<(), RestApiError> {
        let providers = self.providers.read().await;
        if !providers.contains_key(&name) {
            return Err(RestApiError::UnprocessableEntity {
                message: format!("Provider '{}' not found", name)
            });
        }
        drop(providers); // Release read lock before write

        *self.current_provider.write().await = Some(name.clone());
        info!("Switched current provider to: {}", name);
        Ok(())
    }

    // Get current provider name
    pub async fn get_current_provider_name(&self) -> Option<String> {
        self.current_provider.read().await.clone()
    }

    pub async fn get_current_model_name(&self) -> Option<String> {
        let configs = self.configs.read().await;
        let current_name = self.current_provider.read().await.clone()?;

        configs.iter()
            .find(|c| c.name == current_name)
            .map(|c| c.model.clone())
    }

    // Generate response using ONLY the current selected provider - NO FALLBACKS
    pub async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        // Use ONLY the current provider - no fallbacks
        if let Some(current_provider) = self.get_current_provider().await {
            let current_name = self.get_current_provider_name().await.unwrap_or_else(|| "unknown".to_string());
            info!("Using provider: {} with model: {}", current_name, self.get_current_model_name().await.unwrap_or_else(|| "unknown".to_string()));

            match current_provider.generate_response(messages).await {
                Ok(response) => {
                    info!("Successfully got response from provider: {}", current_name);
                    return Ok(response);
                },
                Err(e) => {
                    error!("Provider {} failed: {}. NO FALLBACK - user must select different provider.", current_name, e);
                    return Err(RestApiError::ServiceUnavailable {
                        service: format!("Provider '{}' failed: {}. Please select a different provider.", current_name, e)
                    });
                }
            }
        }

        error!("No provider selected");
        Err(RestApiError::ServiceUnavailable {
            service: "No AI provider selected. Please select a provider first.".to_string()
        })
    }

    // Generate response with specific provider and model
    pub async fn generate_response_with_override(
        &self,
        messages: &[AiChatMessage],
        provider_name: Option<String>,
        model_name: Option<String>
    ) -> Result<String, RestApiError> {
        // If no override specified, use default generation
        if provider_name.is_none() && model_name.is_none() {
            return self.generate_response(messages).await;
        }

        // If provider override specified, use that provider
        if let Some(provider_name) = provider_name {
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&provider_name) {
                // If model override specified, create a new provider instance with that model
                let provider_to_use = if let Some(ref model_override) = model_name {
                    // For Morpheus, we can update the model
                    if provider_name == "morpheus" {
                        if let Ok(api_key) = std::env::var("MORPHEUS_API_KEY") {
                            Arc::new(super::provider::morpheus::MorpheusAdapter::new(api_key, self.http_client.clone())
                                .with_model(model_override.clone()))
                        } else {
                            provider.clone()
                        }
                    } else {
                        // For other providers, we'd need similar logic
                        provider.clone()
                    }
                } else {
                    provider.clone()
                };

                info!("Using override provider: {} with model: {}", provider_name, model_name.as_deref().unwrap_or("default"));

                match provider_to_use.generate_response(messages).await {
                    Ok(response) => {
                        info!("Successfully got response from override provider: {}", provider_name);
                        return Ok(response);
                    },
                    Err(e) => {
                        error!("Override provider {} failed: {}", provider_name, e);
                        return Err(RestApiError::ServiceUnavailable {
                            service: format!("Provider '{}' failed: {}", provider_name, e)
                        });
                    }
                }
            } else {
                return Err(RestApiError::NotFound {
                    resource: format!("Provider '{}' not found", provider_name)
                });
            }
        }

        // If only model override specified, use current provider with new model
        self.generate_response(messages).await
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
    pub async fn update_provider_config(&self, name: &str, config: ProviderConfig) -> Result<(), RestApiError> {
        // Remove old provider
        self.providers.write().await.remove(name);

        // Create new provider with updated config
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
                Arc::new(OpenRouterAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Morpheus => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Morpheus provider requires API key".to_string()
                    })?;
                Arc::new(MorpheusAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Ollama => {
                let base_url = std::env::var("OLLAMA_BASE_URL")
                    .map_err(|_| RestApiError::UnprocessableEntity {
                        message: "Ollama provider requires OLLAMA_BASE_URL environment variable to be set".to_string()
                    })?;
                Arc::new(OllamaAdapter::new(base_url, self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Anthropic => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Anthropic provider requires API key".to_string()
                    })?;
                Arc::new(AnthropicAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::DeepSeek => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "DeepSeek provider requires API key".to_string()
                    })?;
                Arc::new(DeepSeekAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::XAI => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "xAI provider requires API key".to_string()
                    })?;
                Arc::new(XAIAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Gemini => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Gemini provider requires API key".to_string()
                    })?;
                Arc::new(GeminiAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Mistral => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Mistral provider requires API key".to_string()
                    })?;
                Arc::new(MistralAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Together => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Together AI provider requires API key".to_string()
                    })?;
                Arc::new(TogetherAdapter::new(api_key.clone(), self.http_client.clone())
                    .with_model(config.model.clone()))
            },
            ProviderType::Azure => {
                let api_key = config.api_key.as_ref()
                    .ok_or_else(|| RestApiError::UnprocessableEntity {
                        message: "Azure OpenAI provider requires API key".to_string()
                    })?;
                AzureOpenAIAdapter::new(api_key.clone(), self.http_client.clone())
                    .map(|adapter| Arc::new(adapter) as Arc<dyn AiProvider>)?
            },
            ProviderType::Mock => {
                Arc::new(MockAiProvider)
            },
        };

        // Add provider to the providers map
        self.providers.write().await.insert(config.name.clone(), provider);

        // Update configs
        let mut configs = self.configs.write().await;
        // Remove old config
        configs.retain(|c| c.name != name);
        // Add new config
        configs.push(config.clone());
        configs.sort_by_key(|c| c.priority);

        info!("Updated provider: {}", config.name);
        Ok(())
    }

    // Get available models from the current provider
    pub async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        let current_provider = self.get_current_provider().await;

        match current_provider {
            Some(provider) => {
                provider.get_available_models().await
            },
            None => {
                Err(RestApiError::UnprocessableEntity {
                    message: "No provider currently selected".to_string()
                })
            }
        }
    }

    // Update all provider models to use their first available model
    pub async fn update_models_from_api(&mut self) -> Result<(), RestApiError> {
        let mut updated_configs = Vec::new();

        // Get current configs
        let configs = self.configs.read().await.clone();

        for config in configs {
            // Skip mock provider
            if config.provider_type == ProviderType::Mock {
                updated_configs.push(config);
                continue;
            }

            // Get provider and fetch available models
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&config.name) {
                match provider.get_available_models().await {
                    Ok(models) => {
                        if let Some(first_model) = models.first() {
                            let mut updated_config = config.clone();
                            updated_config.model = first_model.clone();
                            updated_configs.push(updated_config);
                            info!("Updated {} provider to use model: {}", config.name, first_model);
                        } else {
                            warn!("No models available for provider: {}", config.name);
                            updated_configs.push(config);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to fetch models for provider {}: {:?}", config.name, e);
                        // Keep original config if model fetching fails
                        updated_configs.push(config);
                    }
                }
            } else {
                updated_configs.push(config);
            }
        }

        // Update configs
        *self.configs.write().await = updated_configs;
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