pub mod provider;
pub mod provider_manager;
pub mod nlp_processor;

use log::{debug, error, info, warn};
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData, EmailMessage, EmailFolder};
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
use serde_json::{json, Value};

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

// Helper struct to hold both string context and structured email data
#[derive(Debug, Clone)]
struct EmailContextData {
    context_string: String,
    email_data: EmailData,
}

pub struct AiService {
    conversations: RwLock<HashMap<String, Conversation>>,
    provider_manager: ProviderManager,
    nlp_processor: NlpProcessor,
    email_service: Option<Arc<super::email::EmailService>>,
    mock_mode: bool, // Flag to force mock responses
    http_client: Client,
    mcp_base_url: String,
    api_key: String,
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
            http_client: Client::new(),
            mcp_base_url: std::env::var("RUSTYMAIL_API_URL")
                .unwrap_or_else(|_| String::new()),
            api_key: std::env::var("RUSTYMAIL_API_KEY")
                .unwrap_or_else(|_| String::new()),
        }
    }

    pub async fn new(
        openai_api_key: Option<String>,
        openrouter_api_key: Option<String>,
        morpheus_api_key: Option<String>,
        ollama_base_url: Option<String>,
        api_key: Option<String>,
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
            http_client: Client::new(),
            mcp_base_url: format!(
                "http://localhost:{}/api",
                std::env::var("REST_PORT")
                    .expect("REST_PORT environment variable must be set")
            ),
            api_key: api_key.unwrap_or_else(||
                std::env::var("RUSTYMAIL_API_KEY")
                    .expect("RUSTYMAIL_API_KEY environment variable must be set")
            ),
        })
    }

    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, ApiError> {
        let conversation_id = query.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let query_text = query.query.clone();
        let provider_override = query.provider_override.clone();
        let model_override = query.model_override.clone();
        let current_folder = query.current_folder.clone();
        let account_id = query.account_id.clone();

        debug!("Processing chatbot query for conversation {}: {} (folder: {:?}, account: {:?})",
               conversation_id, query_text, current_folder, account_id);

        // Always use MCP tools to fetch email context with structured data
        let email_context_data = self.fetch_email_context_with_data(&query_text, account_id.as_deref()).await;

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

            // Add current folder context if available
            if let Some(ref folder) = current_folder {
                system_content.push_str(&format!("\n\nThe user is currently viewing the '{}' folder in their email account.", folder));
            }

            // Add account context if available
            if let Some(ref acc_id) = account_id {
                system_content.push_str(&format!("\n\nThe user is using account ID: {}", acc_id));
            }

            // Add email context if available
            if let Some(ref context_data) = email_context_data {
                system_content.push_str("\n\nCurrent email data from the user's account:\n");
                system_content.push_str(&context_data.context_string);
            }

            messages_history.insert(0, AiChatMessage {
                role: "system".to_string(),
                content: system_content
            });
        }

        let user_message = AiChatMessage { role: "user".to_string(), content: query_text.clone() };
        messages_history.push(user_message.clone());

        // Get provider and model info (use overrides if provided)
        let provider_name = if let Some(ref override_name) = provider_override {
            override_name.clone()
        } else {
            self.provider_manager.get_current_provider_name().await
                .unwrap_or_else(|| "none".to_string())
        };

        let model_name = if let Some(ref override_name) = model_override {
            override_name.clone()
        } else {
            self.provider_manager.get_current_model_name().await
                .unwrap_or_else(|| "none".to_string())
        };

        let response_text = if self.mock_mode {
            warn!("AI Service is in mock mode. Using mock response.");
            format!("[Mock Mode - Provider: mock, Model: mock]\n\n{}", self.generate_mock_response(&query_text))
        } else {
            // Use the override method if overrides are provided
            let response_result = if provider_override.is_some() || model_override.is_some() {
                self.provider_manager.generate_response_with_override(&messages_history, provider_override, model_override).await
            } else {
                self.provider_manager.generate_response(&messages_history).await
            };

            match response_result {
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
            email_data: email_context_data.map(|data| data.email_data),
            followup_suggestions: Some(suggestions),
        })
    }

    // Generate a mock response using MCP tools
    fn generate_mock_response(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();

        // Use async block to get real data via MCP
        let rt = tokio::runtime::Handle::current();

        if query_lower.contains("hello") || query_lower.contains("hi") {
            "Hello! I'm the RustyMail assistant. How can I help you with your emails today?".to_string()
        } else if query_lower.contains("unread") || query_lower.contains("total") || query_lower.contains("how many") {
            // Get real email count from cache via MCP
            match rt.block_on(async {
                self.call_mcp_tool("count_emails_in_folder", json!({"folder": "INBOX"})).await
            }) {
                Ok(result) => {
                    if let Some(count) = result.get("data").and_then(|d| d.get("count")).and_then(|c| c.as_i64()) {
                        format!("You have {} total emails in your inbox. All emails have been synced and cached locally.", count)
                    } else {
                        "I'm having trouble getting the email count. Please try again.".to_string()
                    }
                }
                Err(_) => {
                    "I'm having trouble accessing your emails right now. Please try again.".to_string()
                }
            }
        } else if query_lower.contains("inbox") || query_lower.contains("email") || query_lower.contains("message") {
            match rt.block_on(async {
                // Get folder stats via MCP
                let stats_result = self.call_mcp_tool("get_folder_stats", json!({"folder": "INBOX"})).await?;
                let stats_data = stats_result.get("data").unwrap_or(&stats_result);
                let total = stats_data.get("total_emails").and_then(|t| t.as_i64()).unwrap_or(0);
                let unread = stats_data.get("unread_count").and_then(|u| u.as_i64()).unwrap_or(0);

                // Get recent emails via MCP
                let emails_result = self.call_mcp_tool("list_cached_emails", json!({
                    "folder": "INBOX",
                    "limit": 8,
                    "offset": 0
                })).await?;

                let shown = emails_result.get("data")
                    .and_then(|e| e.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                Ok::<(i64, i64, usize), String>((total, unread, shown))
            }) {
                Ok((total, unread, shown)) => {
                    format!("Your inbox contains {} total emails ({} unread). I can show you the most recent {} emails. All emails have been synced to your local cache.", total, unread, shown)
                }
                Err(_) => {
                    "I'm having trouble accessing your emails right now. Please try again.".to_string()
                }
            }
        } else if query_lower.contains("sent") {
            "Your Sent folder functionality is coming soon.".to_string()
        } else {
            "I can help you with your emails. You can ask me about your inbox, unread messages, or search for specific emails.".to_string()
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

    /// Call an MCP tool through the HTTP API
    async fn call_mcp_tool(&self, tool_name: &str, args: Value) -> Result<Value, String> {
        let url = format!("{}/dashboard/mcp/execute", self.mcp_base_url);

        let body = json!({
            "tool": tool_name,
            "parameters": args
        });

        match self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    response.json::<Value>().await
                        .map_err(|e| format!("Failed to parse MCP response: {}", e))
                } else {
                    Err(format!("MCP tool failed with status: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to call MCP tool: {}", e))
        }
    }

    /// Fetch email context using MCP tools
    async fn fetch_email_context_mcp(&self, query: &str, account_id: Option<&str>) -> Option<String> {
        let query_lower = query.to_lowercase();

        // Check if query is about folders
        if query_lower.contains("folder") || query_lower.contains("mailbox") {
            let mut params = json!({});
            if let Some(acc_id) = account_id {
                params["account_id"] = json!(acc_id);
            }
            match self.call_mcp_tool("list_folders", params).await {
                Ok(result) => {
                    if let Some(folders) = result.get("data").and_then(|d| d.as_array()) {
                        let folder_names: Vec<String> = folders.iter()
                            .filter_map(|f| f.as_str())
                            .map(|s| s.to_string())
                            .collect();

                        let context = format!("Your email account has {} folders:\n{}",
                            folder_names.len(),
                            folder_names.join("\n"));
                        return Some(context);
                    }
                },
                Err(e) => {
                    error!("Failed to list folders via MCP: {}", e);
                }
            }
        }

        // Get total email count first
        let mut count_params = json!({"folder": "INBOX"});
        if let Some(acc_id) = account_id {
            count_params["account_id"] = json!(acc_id);
        }
        let total_count = match self.call_mcp_tool("count_emails_in_folder", count_params).await {
            Ok(result) => result.get("data")
                .and_then(|d| d.get("count"))
                .and_then(|c| c.as_i64())
                .unwrap_or(0),
            Err(e) => {
                error!("Failed to get email count via MCP: {}", e);
                return None;
            }
        };

        // Get recent emails
        let mut list_params = json!({
            "folder": "INBOX",
            "limit": 10,
            "offset": 0
        });
        if let Some(acc_id) = account_id {
            list_params["account_id"] = json!(acc_id);
        }
        let emails_result = match self.call_mcp_tool("list_cached_emails", list_params).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to list emails via MCP: {}", e);
                return None;
            }
        };

        // Build context from MCP response
        if let Some(emails) = emails_result.get("data").and_then(|e| e.as_array()) {
            let mut context = format!("You have {} total emails in your inbox.\n", total_count);

            if !emails.is_empty() {
                context.push_str(&format!("Here are the {} most recent emails:\n", emails.len()));

                for (i, email) in emails.iter().enumerate() {
                    let subject = email.get("subject")
                        .and_then(|s| s.as_str())
                        .unwrap_or("(No subject)");
                    let from = email.get("from_address")
                        .and_then(|f| f.as_str())
                        .unwrap_or("Unknown");
                    let date = email.get("date")
                        .and_then(|d| d.as_str())
                        .or_else(|| email.get("internal_date").and_then(|d| d.as_str()))
                        .unwrap_or("Unknown date");
                    let flags = email.get("flags")
                        .and_then(|f| f.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>())
                        .unwrap_or_default();
                    let unread = !flags.iter().any(|f| f.contains("Seen"));

                    context.push_str(&format!(
                        "{}. Subject: {}, From: {}, Date: {}{}\n",
                        i + 1, subject, from, date,
                        if unread { " [UNREAD]" } else { "" }
                    ));
                }
            }

            Some(context)
        } else {
            None
        }
    }

    /// Fetch email context with structured data for the chatbot
    async fn fetch_email_context_with_data(&self, query: &str, account_id: Option<&str>) -> Option<EmailContextData> {
        let query_lower = query.to_lowercase();
        let mut email_messages: Vec<EmailMessage> = Vec::new();
        let mut email_folders: Vec<EmailFolder> = Vec::new();
        let mut total_count: Option<u32> = None;

        // Check if query is about folders
        if query_lower.contains("folder") || query_lower.contains("mailbox") {
            let mut params = json!({});
            if let Some(acc_id) = account_id {
                params["account_id"] = json!(acc_id);
            }
            match self.call_mcp_tool("list_folders", params).await {
                Ok(result) => {
                    if let Some(folders) = result.get("data").and_then(|d| d.as_array()) {
                        for folder_name in folders.iter().filter_map(|f| f.as_str()) {
                            // Get folder stats for each folder
                            let mut folder_params = json!({"folder": folder_name});
                            if let Some(acc_id) = account_id {
                                folder_params["account_id"] = json!(acc_id);
                            }

                            let count = match self.call_mcp_tool("count_emails_in_folder", folder_params).await {
                                Ok(count_result) => count_result.get("data")
                                    .and_then(|d| d.get("count"))
                                    .and_then(|c| c.as_u64())
                                    .unwrap_or(0) as u32,
                                Err(_) => 0,
                            };

                            email_folders.push(EmailFolder {
                                name: folder_name.to_string(),
                                count,
                                unread_count: 0, // MCP doesn't provide unread count per folder yet
                            });
                        }

                        let context = format!("Your email account has {} folders:\n{}",
                            email_folders.len(),
                            email_folders.iter().map(|f| format!("{} ({} emails)", f.name, f.count)).collect::<Vec<_>>().join("\n"));

                        return Some(EmailContextData {
                            context_string: context,
                            email_data: EmailData {
                                messages: None,
                                count: None,
                                folders: Some(email_folders),
                            },
                        });
                    }
                },
                Err(e) => {
                    error!("Failed to list folders via MCP: {}", e);
                    return None;
                }
            }
        }

        // Get total email count
        let mut count_params = json!({"folder": "INBOX"});
        if let Some(acc_id) = account_id {
            count_params["account_id"] = json!(acc_id);
        }
        total_count = match self.call_mcp_tool("count_emails_in_folder", count_params).await {
            Ok(result) => result.get("data")
                .and_then(|d| d.get("count"))
                .and_then(|c| c.as_u64())
                .map(|c| c as u32),
            Err(e) => {
                error!("Failed to get email count via MCP: {}", e);
                return None;
            }
        };

        // Get recent emails
        let mut list_params = json!({
            "folder": "INBOX",
            "limit": 10,
            "offset": 0
        });
        if let Some(acc_id) = account_id {
            list_params["account_id"] = json!(acc_id);
        }
        let emails_result = match self.call_mcp_tool("list_cached_emails", list_params).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to list emails via MCP: {}", e);
                return None;
            }
        };

        // Parse emails into EmailMessage structs
        if let Some(emails) = emails_result.get("data").and_then(|e| e.as_array()) {
            for email in emails.iter() {
                let id = email.get("id")
                    .or_else(|| email.get("uid"))
                    .and_then(|i| i.as_u64())
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "0".to_string());

                let subject = email.get("subject")
                    .and_then(|s| s.as_str())
                    .unwrap_or("(No subject)")
                    .to_string();

                let from = email.get("from_address")
                    .and_then(|f| f.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let date = email.get("date")
                    .and_then(|d| d.as_str())
                    .or_else(|| email.get("internal_date").and_then(|d| d.as_str()))
                    .unwrap_or("Unknown date")
                    .to_string();

                let flags = email.get("flags")
                    .and_then(|f| f.as_array())
                    .map(|arr| arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>())
                    .unwrap_or_default();

                let is_read = flags.iter().any(|f| f.contains("Seen"));

                // Extract snippet from body or preview
                let snippet = email.get("body_preview")
                    .or_else(|| email.get("snippet"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(150)
                    .collect::<String>();

                email_messages.push(EmailMessage {
                    id,
                    subject,
                    from,
                    date,
                    snippet,
                    is_read,
                });
            }

            let mut context = format!("You have {} total emails in your inbox.\n", total_count.unwrap_or(0));
            if !email_messages.is_empty() {
                context.push_str(&format!("Here are the {} most recent emails:\n", email_messages.len()));
                for (i, msg) in email_messages.iter().enumerate() {
                    context.push_str(&format!(
                        "{}. Subject: {}, From: {}, Date: {}{}\n",
                        i + 1, msg.subject, msg.from, msg.date,
                        if !msg.is_read { " [UNREAD]" } else { "" }
                    ));
                }
            }

            Some(EmailContextData {
                context_string: context,
                email_data: EmailData {
                    messages: Some(email_messages),
                    count: total_count,
                    folders: None,
                },
            })
        } else {
            None
        }
    }

    /// Fetch email context based on the query (legacy - uses direct email service)
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
