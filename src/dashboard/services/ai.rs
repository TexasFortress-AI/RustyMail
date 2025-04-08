use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData, EmailMessage, EmailFolder};
use crate::imap::ImapClient;
use std::sync::Arc;
use uuid::Uuid;
use log::{info, debug, error};
use std::collections::HashMap;
use tokio::sync::Mutex;
use chrono::Utc;

// Simple in-memory conversation store
type ConversationStore = HashMap<String, Vec<String>>;

pub struct AiService {
    imap_client: Arc<ImapClient>,
    conversations: Mutex<ConversationStore>,
}

impl AiService {
    pub fn new(imap_client: Arc<ImapClient>) -> Self {
        Self {
            imap_client,
            conversations: Mutex::new(HashMap::new()),
        }
    }
    
    pub async fn process_query(&self, query: ChatbotQuery) -> Result<ChatbotResponse, String> {
        let conversation_id = query.conversation_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        
        // Store query in conversation history
        {
            let mut conversations = self.conversations.lock().await;
            let history = conversations.entry(conversation_id.clone()).or_insert_with(Vec::new);
            history.push(query.query.clone());
            
            // Limit conversation history
            if history.len() > 10 {
                history.remove(0);
            }
        }
        
        // Process the query and generate a response
        let (response_text, email_data, followup_suggestions) = 
            self.generate_response(&query.query, &conversation_id).await?;
        
        Ok(ChatbotResponse {
            text: response_text,
            conversation_id,
            email_data,
            followup_suggestions,
        })
    }
    
    async fn generate_response(
        &self,
        query: &str,
        conversation_id: &str,
    ) -> Result<(String, Option<EmailData>, Option<Vec<String>>), String> {
        // Get conversation history
        let history = {
            let conversations = self.conversations.lock().await;
            conversations.get(conversation_id)
                .map(|h| h.clone())
                .unwrap_or_default()
        };
        
        // Convert query to lowercase for easier matching
        let query_lower = query.to_lowercase();
        
        // Simple rule-based responses
        if query_lower.contains("hello") || query_lower.contains("hi") {
            return Ok((
                "Hello! I'm the RustyMail assistant. How can I help you with your emails today?".to_string(),
                None,
                Some(vec![
                    "Show me my unread emails".to_string(),
                    "How many emails are in my inbox?".to_string(),
                    "Show me my folders".to_string(),
                ]),
            ));
        }
        
        // Handle email count queries
        if query_lower.contains("how many") && (query_lower.contains("email") || query_lower.contains("message")) {
            // In a real implementation, we would query the IMAP server
            // For now, we'll return a mock response
            let folder = if query_lower.contains("inbox") {
                "INBOX"
            } else if query_lower.contains("sent") {
                "Sent"
            } else {
                "INBOX"
            };
            
            let unread_count = 12; // Mock value
            let total_count = 42; // Mock value
            
            return Ok((
                format!("You have {} emails in {}, including {} unread messages.", 
                        total_count, folder, unread_count),
                Some(EmailData {
                    count: Some(total_count),
                    messages: None,
                    folders: None,
                }),
                Some(vec![
                    "Show me my unread emails".to_string(),
                    "When was the last email received?".to_string(),
                ]),
            ));
        }
        
        // Handle folder listing
        if query_lower.contains("folder") && (query_lower.contains("show") || query_lower.contains("list")) {
            // Mock folders
            let folders = vec![
                EmailFolder { 
                    name: "INBOX".to_string(), 
                    count: 42, 
                    unread_count: 12 
                },
                EmailFolder { 
                    name: "Sent".to_string(), 
                    count: 18, 
                    unread_count: 0 
                },
                EmailFolder { 
                    name: "Drafts".to_string(), 
                    count: 3, 
                    unread_count: 0 
                },
                EmailFolder { 
                    name: "Spam".to_string(), 
                    count: 7, 
                    unread_count: 7 
                },
            ];
            
            return Ok((
                "Here are your email folders:".to_string(),
                Some(EmailData {
                    count: None,
                    messages: None,
                    folders: Some(folders),
                }),
                Some(vec![
                    "Show me emails in INBOX".to_string(),
                    "How many unread emails do I have?".to_string(),
                ]),
            ));
        }
        
        // Handle showing emails
        if (query_lower.contains("show") || query_lower.contains("list")) && 
           (query_lower.contains("email") || query_lower.contains("message")) {
            let folder = if query_lower.contains("inbox") {
                "INBOX"
            } else if query_lower.contains("sent") {
                "Sent"
            } else {
                "INBOX"
            };
            
            // Create mock messages
            let messages = vec![
                EmailMessage {
                    id: "1".to_string(),
                    subject: "Weekly Team Meeting".to_string(),
                    from: "team@example.com".to_string(),
                    date: Utc::now().to_rfc3339(),
                    snippet: "Let's discuss project progress...".to_string(),
                    is_read: false,
                },
                EmailMessage {
                    id: "2".to_string(),
                    subject: "Your Account Statement".to_string(),
                    from: "bank@example.com".to_string(),
                    date: Utc::now().to_rfc3339(),
                    snippet: "Your monthly statement is ready...".to_string(),
                    is_read: true,
                },
                EmailMessage {
                    id: "3".to_string(),
                    subject: "Vacation Plans".to_string(),
                    from: "friend@example.com".to_string(),
                    date: Utc::now().to_rfc3339(),
                    snippet: "I was thinking about our summer plans...".to_string(),
                    is_read: false,
                },
            ];
            
            return Ok((
                format!("Here are the most recent emails in {}:", folder),
                Some(EmailData {
                    count: Some(messages.len() as u32),
                    messages: Some(messages),
                    folders: None,
                }),
                Some(vec![
                    "Show me my unread emails only".to_string(),
                    "How many emails do I have total?".to_string(),
                ]),
            ));
        }
        
        // Default fallback response
        Ok((
            "I'm not sure how to help with that. You can ask me about your emails, folders, or message counts.".to_string(),
            None,
            Some(vec![
                "Show me my recent emails".to_string(),
                "How many unread emails do I have?".to_string(),
                "Show me my folders".to_string(),
            ]),
        ))
    }
}
