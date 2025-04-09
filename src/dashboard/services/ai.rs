use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData};
use log::{debug, warn, error};
use std::sync::Arc;

// Import the provider trait and message struct
mod providers;
use providers::{AiProvider, AiChatMessage};

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

#[derive(Debug)]
pub struct AiService {
    conversations: RwLock<HashMap<String, Conversation>>,
    provider: Arc<dyn AiProvider>, // Use the trait object
    mock_mode: bool, // Flag to force mock responses
}

impl AiService {
    pub fn new(provider: Arc<dyn AiProvider>, force_mock: bool) -> Self {
        Self {
            conversations: RwLock::new(HashMap::new()),
            provider,
            mock_mode: force_mock,
        }
    }

    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, ApiError> {
        let conversation_id = query.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let query_text = query.query.clone();
        
        debug!("Processing chatbot query for conversation {}: {}", conversation_id, query_text);
        
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
        
        // Update last activity time
        conversation.last_activity = chrono::Utc::now();

        // Prepare the message history for OpenAI
        let mut messages_history: Vec<AiChatMessage> = conversation.entries.iter()
            .map(|entry| entry.message.clone()) // Clone messages from history
            .collect();
        
        // Add the current user query
        let user_message = AiChatMessage { role: "user".to_string(), content: query_text.clone() };
        messages_history.push(user_message.clone());
        
        // Generate response: Use API key if available, otherwise use mock logic
        let response_text_result = if self.mock_mode {
            warn!("AI Service is in mock mode. Using mock response.");
            Ok(self.generate_mock_response(&query_text))
        } else {
            // Call the provider via the trait object
            self.provider.generate_response(&messages_history).await
        };

        let response_text = match response_text_result {
            Ok(text) => text,
            Err(e) => {
                error!("AI Service failed to get response: {}. Falling back to mock.", e);
                self.generate_mock_response(&query_text) // Fallback to mock
            }
        };

        // Store user query and AI response in conversation history
        let assistant_message = AiChatMessage { role: "assistant".to_string(), content: response_text.clone() };
        conversation.entries.push(ConversationEntry {
            message: user_message, // Store user query
            timestamp: chrono::Utc::now(),
        });
         conversation.entries.push(ConversationEntry {
            message: assistant_message, // Store assistant response
            timestamp: chrono::Utc::now(),
        });
        
        Ok(ChatbotResponse {
            text: response_text,
            conversation_id,
            email_data: None, // Keep email_data logic separate for now
            followup_suggestions: Some(vec![
                "Show me my unread emails".to_string(),
                "How many emails do I have from support?".to_string(),
                "What's in my Sent folder?".to_string(),
            ]),
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
