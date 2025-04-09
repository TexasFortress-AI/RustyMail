use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse, EmailData, EmailMessage, EmailFolder};
use log::{debug, warn, error};
use reqwest::Client;
use serde::{Serialize, Deserialize};
use crate::dashboard::api::errors::ApiError;

// OpenAI API constants
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const OPENAI_MODEL: &str = "gpt-4o-mini"; // Specify the desired model

// OpenAI API request/response structures
#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    // Add other parameters like temperature, max_tokens if needed
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    // Add usage, error fields if needed for more detailed handling
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    // Add finish_reason if needed
}

// Conversation history entry
#[derive(Debug, Clone)]
struct ConversationEntry {
    message: OpenAiMessage, // Store as OpenAiMessage for history context
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
    http_client: Client, // Use reqwest::Client
}

impl AiService {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            conversations: RwLock::new(HashMap::new()),
            api_key,
            http_client: Client::new(), // Create a reqwest client instance
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
        let mut messages_history: Vec<OpenAiMessage> = conversation.entries.iter()
            .map(|entry| entry.message.clone()) // Clone messages from history
            .collect();
        
        // Add the current user query
        let user_message = OpenAiMessage { role: "user".to_string(), content: query_text.clone() };
        messages_history.push(user_message.clone());
        
        // Generate response: Use API key if available, otherwise use mock logic
        let response_text_result = match &self.api_key {
            Some(key) => {
                self.call_openai_api(key, &messages_history).await
            }
            None => {
                warn!("No OpenAI API key configured. Using mock AI response.");
                Ok(self.generate_mock_response(&query_text))
            }
        };

        let response_text = match response_text_result {
            Ok(text) => text,
            Err(e) => {
                error!("AI Service failed to get response: {}. Falling back to mock.", e);
                // Optionally, return the error instead of falling back
                // return Err(e); 
                self.generate_mock_response(&query_text) // Fallback to mock
            }
        };

        // Store user query and AI response in conversation history
        let assistant_message = OpenAiMessage { role: "assistant".to_string(), content: response_text.clone() };
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

    // Helper function to call OpenAI API
    async fn call_openai_api(&self, api_key: &str, messages: &[OpenAiMessage]) -> Result<String, ApiError> {
        let request_payload = OpenAiRequest {
            model: OPENAI_MODEL.to_string(),
            messages: messages.to_vec(),
        };

        debug!("Sending request to OpenAI API: model={}, messages_count={}", 
               request_payload.model, request_payload.messages.len());

        let response = self.http_client
            .post(OPENAI_API_URL)
            .bearer_auth(api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30)) // Add timeout
            .send()
            .await
            .map_err(|e| ApiError::AiRequestError(format!("Network error calling OpenAI: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("OpenAI API request failed with status {}: {}", status, error_body);
            return Err(ApiError::AiRequestError(format!(
                "OpenAI API returned error status {}: {}",
                status,
                error_body
            )));
        }

        let response_body = response
            .json::<OpenAiResponse>()
            .await
            .map_err(|e| ApiError::AiServiceError(format!("Failed to deserialize OpenAI response: {}", e)))?;

        // Extract the first choice's message content
        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from OpenAI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("OpenAI API response did not contain any choices.");
            Err(ApiError::AiServiceError("OpenAI response was empty or missing choices".to_string()))
        }
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
