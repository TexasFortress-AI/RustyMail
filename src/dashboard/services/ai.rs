use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData, EmailMessage, EmailFolder};
use log::debug;
use chrono::Utc;
use std::sync::Arc;

// Conversation history entry
#[derive(Debug, Clone)]
struct ConversationEntry {
    query: String,
    response: String,
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
    api_key: Option<String>,
}

impl AiService {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            conversations: RwLock::new(HashMap::new()),
            api_key,
        }
    }

    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, String> {
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
        
        // Generate response based on API key
        let response_text = if let Some(key) = &self.api_key {
            format!("AI response using key {}", key)
        } else {
            "No API key configured".to_string()
        };
        
        // Add to conversation history
        conversation.entries.push(ConversationEntry {
            query: query_text,
            response: response_text.clone(),
            timestamp: chrono::Utc::now(),
        });
        
        Ok(ChatbotResponse {
            text: response_text,
            conversation_id,
            email_data: Some(self.generate_mock_email_data()),
            followup_suggestions: Some(vec![
                "Show me my unread emails".to_string(),
                "How many emails do I have from support?".to_string(),
                "What's in my Sent folder?".to_string(),
            ]),
        })
    }
    
    // Generate a mock response for testing
    fn generate_mock_response(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        
        if query_lower.contains("hello") || query_lower.contains("hi") {
            "Hello! I'm the RustyMail assistant. How can I help you with your emails today?".to_string()
        } else if query_lower.contains("unread") {
            "You have 3 unread emails in your inbox. Would you like me to show them to you?".to_string()
        } else if query_lower.contains("inbox") {
            "Your inbox contains 24 messages total, with 3 unread. The most recent message is from support@example.com about 'Your recent inquiry'.".to_string()
        } else if query_lower.contains("sent") {
            "Your Sent folder contains 12 messages. The most recent was sent to contact@example.com about 'Project status update'.".to_string()
        } else {
            "I'm not sure how to respond to that yet. I'm just a simulated AI assistant for development purposes. In the real implementation, I would use RIG or OpenAI to generate helpful responses about your emails.".to_string()
        }
    }
    
    // Generate mock email data for testing
    fn generate_mock_email_data(&self) -> EmailData {
        EmailData {
            messages: Some(vec![
                EmailMessage {
                    id: "1".to_string(),
                    subject: "Welcome to RustyMail".to_string(),
                    from: "system@rustymail.com".to_string(),
                    date: chrono::Utc::now().to_rfc3339(),
                    snippet: "Welcome to RustyMail! Here's how to get started...".to_string(),
                    is_read: false,
                },
                EmailMessage {
                    id: "2".to_string(),
                    subject: "Your account settings".to_string(),
                    from: "support@rustymail.com".to_string(),
                    date: chrono::Utc::now().to_rfc3339(),
                    snippet: "Your account has been configured with...".to_string(),
                    is_read: true,
                }
            ]),
            count: Some(2),
            folders: Some(vec![
                EmailFolder {
                    name: "INBOX".to_string(),
                    count: 2,
                    unread_count: 1,
                },
                EmailFolder {
                    name: "Sent".to_string(),
                    count: 5,
                    unread_count: 0,
                }
            ]),
        }
    }
    
    // Clean up old conversations
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
