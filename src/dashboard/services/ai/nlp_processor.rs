// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/nlp_processor.rs
// Natural Language Processing Pipeline for converting user queries to MCP operations

use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use log::{debug, info, warn, error};
use std::collections::HashMap;
use crate::api::errors::ApiError as RestApiError;
use super::provider::{AiProvider, AiChatMessage};
use super::provider_manager::{ProviderManager, ConversationContext};

use std::fmt;

// Intent types that map to MCP operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmailIntent {
    // Folder operations
    ListFolders,
    CreateFolder(String),
    RenameFolder { old_name: String, new_name: String },
    DeleteFolder(String),

    // Email operations
    ListEmails { folder: Option<String>, limit: Option<usize> },
    SearchEmails { query: String, folder: Option<String> },
    ShowUnreadEmails,
    ShowEmailsFromSender(String),
    MoveEmailsToFolder { from_folder: String, to_folder: String, criteria: String },
    DeleteEmails { folder: String, criteria: String },
    MarkAsRead { folder: String, criteria: String },
    MarkAsUnread { folder: String, criteria: String },

    // General queries
    GetEmailCount { folder: Option<String> },
    ShowRecentEmails { count: usize },

    // Unknown/Help
    Unknown,
    Help,
}

impl fmt::Display for EmailIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmailIntent::ListFolders => write!(f, "list folders"),
            EmailIntent::CreateFolder(name) => write!(f, "create folder '{}'", name),
            EmailIntent::RenameFolder { old_name, new_name } => write!(f, "rename folder '{}' to '{}'", old_name, new_name),
            EmailIntent::DeleteFolder(name) => write!(f, "delete folder '{}'", name),
            EmailIntent::ListEmails { folder, limit } => {
                let folder_str = folder.as_deref().unwrap_or("INBOX");
                if let Some(l) = limit {
                    write!(f, "list {} emails from {}", l, folder_str)
                } else {
                    write!(f, "list emails from {}", folder_str)
                }
            },
            EmailIntent::SearchEmails { query, folder } => {
                let folder_str = folder.as_deref().unwrap_or("all folders");
                write!(f, "search for '{}' in {}", query, folder_str)
            },
            EmailIntent::ShowUnreadEmails => write!(f, "show unread emails"),
            EmailIntent::ShowEmailsFromSender(sender) => write!(f, "show emails from {}", sender),
            EmailIntent::MoveEmailsToFolder { from_folder, to_folder, criteria } => {
                write!(f, "move emails matching '{}' from {} to {}", criteria, from_folder, to_folder)
            },
            EmailIntent::DeleteEmails { folder, criteria } => write!(f, "delete emails matching '{}' in {}", criteria, folder),
            EmailIntent::MarkAsRead { folder, criteria } => write!(f, "mark emails matching '{}' as read in {}", criteria, folder),
            EmailIntent::MarkAsUnread { folder, criteria } => write!(f, "mark emails matching '{}' as unread in {}", criteria, folder),
            EmailIntent::GetEmailCount { folder } => {
                let folder_str = folder.as_deref().unwrap_or("all folders");
                write!(f, "get email count in {}", folder_str)
            },
            EmailIntent::ShowRecentEmails { count } => write!(f, "show {} recent emails", count),
            EmailIntent::Unknown => write!(f, "unknown intent"),
            EmailIntent::Help => write!(f, "help"),
        }
    }
}

// Extracted entities from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntities {
    pub folders: Vec<String>,
    pub senders: Vec<String>,
    pub subjects: Vec<String>,
    pub dates: Vec<String>,
    pub flags: Vec<String>,
    pub counts: Vec<usize>,
    pub search_terms: Vec<String>,
}

// NLP processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpResult {
    pub intent: EmailIntent,
    pub entities: ExtractedEntities,
    pub confidence: f32,
    pub original_query: String,
    pub mcp_operation: Option<McpOperation>,
}

// MCP operation mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpOperation {
    pub method: String,
    pub params: serde_json::Value,
}

// NLP Processor for converting natural language to MCP operations
pub struct NlpProcessor {
    provider_manager: ProviderManager,
    prompt_templates: HashMap<String, String>,
    intent_patterns: Vec<(regex::Regex, EmailIntent)>,
}

impl NlpProcessor {
    pub fn new(provider_manager: ProviderManager) -> Self {
        let prompt_templates = Self::init_prompt_templates();
        let intent_patterns = Self::init_intent_patterns();

        Self {
            provider_manager,
            prompt_templates,
            intent_patterns,
        }
    }

    // Initialize prompt templates for different operations
    fn init_prompt_templates() -> HashMap<String, String> {
        let mut templates = HashMap::new();

        // Intent extraction template
        templates.insert("intent_extraction".to_string(), r#"
You are an email assistant. Analyze the user's query and extract the intent and entities.

User Query: {query}

Extract the following:
1. Intent (one of: list_folders, create_folder, rename_folder, delete_folder, list_emails, search_emails, show_unread, show_from_sender, move_emails, delete_emails, mark_read, mark_unread, get_count, show_recent, help, unknown)
2. Entities:
   - Folders mentioned
   - Email senders
   - Subject keywords
   - Date references
   - Email flags (read/unread/flagged/etc)
   - Numbers/counts
   - Search terms

Respond in JSON format:
{
  "intent": "intent_name",
  "entities": {
    "folders": [],
    "senders": [],
    "subjects": [],
    "dates": [],
    "flags": [],
    "counts": [],
    "search_terms": []
  }
}
"#.to_string());

        // MCP mapping template
        templates.insert("mcp_mapping".to_string(), r#"
Convert the following email operation intent into an MCP (Model Context Protocol) method call.

Intent: {intent}
Entities: {entities}

Available MCP methods:
- list_folders()
- create_folder(name: String)
- rename_folder(old_name: String, new_name: String)
- delete_folder(name: String)
- list_emails(folder: String, limit: Option<usize>)
- search_emails(query: String, folder: Option<String>)
- move_email(from_folder: String, to_folder: String, uid: u32)
- delete_email(folder: String, uid: u32)
- mark_as_read(folder: String, uid: u32)
- mark_as_unread(folder: String, uid: u32)

Respond with the MCP method and parameters in JSON:
{
  "method": "method_name",
  "params": {}
}
"#.to_string());

        templates
    }

    // Initialize regex patterns for quick intent detection
    fn init_intent_patterns() -> Vec<(regex::Regex, EmailIntent)> {
        vec![
            // Folder operations
            (regex::Regex::new(r"(?i)(list|show|display).*(folder|mailbox)").unwrap(),
             EmailIntent::ListFolders),
            (regex::Regex::new(r"(?i)create.*(folder|mailbox).*named?\s+(.+)").unwrap(),
             EmailIntent::Unknown), // Will be refined with entity extraction
            (regex::Regex::new(r"(?i)rename.*(folder|mailbox)").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)delete.*(folder|mailbox)").unwrap(),
             EmailIntent::Unknown),

            // Email operations
            (regex::Regex::new(r"(?i)(show|list|display).*(unread|new)\s*(email|message)").unwrap(),
             EmailIntent::ShowUnreadEmails),
            (regex::Regex::new(r"(?i)(email|message).*(from|sender)").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)(search|find).*(email|message)").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)move.*(email|message)").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)delete.*(email|message)").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)mark.*as\s*(read|unread)").unwrap(),
             EmailIntent::Unknown),

            // General
            (regex::Regex::new(r"(?i)how\s+many").unwrap(),
             EmailIntent::Unknown),
            (regex::Regex::new(r"(?i)(help|what can you do)").unwrap(),
             EmailIntent::Help),
        ]
    }

    // Process natural language query
    pub async fn process_query(&self, query: &str, context: Option<&ConversationContext>) -> Result<NlpResult, RestApiError> {
        info!("Processing NLP query: {}", query);

        // First, try pattern matching for quick intent detection
        let preliminary_intent = self.detect_intent_by_pattern(query);
        debug!("Preliminary intent: {:?}", preliminary_intent);

        // Use AI provider for more sophisticated extraction
        let extraction_result = self.extract_with_ai(query, context).await?;

        // Map to MCP operation
        let mcp_operation = self.map_to_mcp_operation(&extraction_result.intent, &extraction_result.entities)?;

        Ok(NlpResult {
            intent: extraction_result.intent,
            entities: extraction_result.entities,
            confidence: extraction_result.confidence,
            original_query: query.to_string(),
            mcp_operation: Some(mcp_operation),
        })
    }

    // Quick pattern-based intent detection
    fn detect_intent_by_pattern(&self, query: &str) -> EmailIntent {
        for (pattern, intent) in &self.intent_patterns {
            if pattern.is_match(query) {
                return intent.clone();
            }
        }
        EmailIntent::Unknown
    }

    // Extract intent and entities using AI
    async fn extract_with_ai(&self, query: &str, context: Option<&ConversationContext>) -> Result<NlpResult, RestApiError> {
        let prompt_template = self.prompt_templates.get("intent_extraction")
            .ok_or_else(|| RestApiError::InternalError {
                message: "Intent extraction template not found".to_string()
            })?;

        let prompt = prompt_template.replace("{query}", query);

        // Build message history with context if available
        let mut messages = vec![];
        if let Some(ctx) = context {
            // Add recent context messages
            for msg in ctx.get_messages().iter().rev().take(3).rev() {
                messages.push(msg.clone());
            }
        }

        // Add system message
        messages.push(AiChatMessage {
            role: "system".to_string(),
            content: "You are an email assistant that extracts intents and entities from user queries.".to_string(),
        });

        // Add user query
        messages.push(AiChatMessage {
            role: "user".to_string(),
            content: prompt,
        });

        // Get AI response
        let response = self.provider_manager.generate_response(&messages).await?;

        // Parse JSON response
        self.parse_extraction_response(&response, query)
    }

    // Parse AI extraction response
    fn parse_extraction_response(&self, response: &str, original_query: &str) -> Result<NlpResult, RestApiError> {
        // Try to extract JSON from response
        let json_start = response.find('{');
        let json_end = response.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &response[start..=end];

            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(json) => {
                    let intent = self.parse_intent_from_json(&json);
                    let entities = self.parse_entities_from_json(&json);

                    Ok(NlpResult {
                        intent,
                        entities,
                        confidence: 0.8, // Default confidence
                        original_query: original_query.to_string(),
                        mcp_operation: None,
                    })
                },
                Err(e) => {
                    warn!("Failed to parse AI response as JSON: {}", e);
                    // Fallback to pattern detection
                    Ok(NlpResult {
                        intent: self.detect_intent_by_pattern(original_query),
                        entities: ExtractedEntities::default(),
                        confidence: 0.5,
                        original_query: original_query.to_string(),
                        mcp_operation: None,
                    })
                }
            }
        } else {
            // No JSON found, use fallback
            Ok(NlpResult {
                intent: self.detect_intent_by_pattern(original_query),
                entities: ExtractedEntities::default(),
                confidence: 0.5,
                original_query: original_query.to_string(),
                mcp_operation: None,
            })
        }
    }

    // Parse intent from JSON
    fn parse_intent_from_json(&self, json: &serde_json::Value) -> EmailIntent {
        if let Some(intent_str) = json.get("intent").and_then(|v| v.as_str()) {
            match intent_str {
                "list_folders" => EmailIntent::ListFolders,
                "show_unread" => EmailIntent::ShowUnreadEmails,
                "help" => EmailIntent::Help,
                // Add more mappings
                _ => EmailIntent::Unknown,
            }
        } else {
            EmailIntent::Unknown
        }
    }

    // Parse entities from JSON
    fn parse_entities_from_json(&self, json: &serde_json::Value) -> ExtractedEntities {
        let default_entities = serde_json::json!({});
        let entities = json.get("entities").unwrap_or(&default_entities);

        ExtractedEntities {
            folders: self.extract_string_array(entities, "folders"),
            senders: self.extract_string_array(entities, "senders"),
            subjects: self.extract_string_array(entities, "subjects"),
            dates: self.extract_string_array(entities, "dates"),
            flags: self.extract_string_array(entities, "flags"),
            counts: self.extract_number_array(entities, "counts"),
            search_terms: self.extract_string_array(entities, "search_terms"),
        }
    }

    // Helper to extract string arrays from JSON
    fn extract_string_array(&self, json: &serde_json::Value, key: &str) -> Vec<String> {
        json.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    // Helper to extract number arrays from JSON
    fn extract_number_array(&self, json: &serde_json::Value, key: &str) -> Vec<usize> {
        json.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default()
    }

    // Map intent and entities to MCP operation
    fn map_to_mcp_operation(&self, intent: &EmailIntent, entities: &ExtractedEntities) -> Result<McpOperation, RestApiError> {
        let (method, params) = match intent {
            EmailIntent::ListFolders => {
                ("list_folders".to_string(), serde_json::json!({}))
            },
            EmailIntent::ShowUnreadEmails => {
                let folder = entities.folders.first()
                    .unwrap_or(&"INBOX".to_string())
                    .clone();
                ("search_emails".to_string(), serde_json::json!({
                    "folder": folder,
                    "query": "UNSEEN"
                }))
            },
            EmailIntent::ShowEmailsFromSender(sender) => {
                let folder = entities.folders.first()
                    .unwrap_or(&"INBOX".to_string())
                    .clone();
                ("search_emails".to_string(), serde_json::json!({
                    "folder": folder,
                    "query": format!("FROM \"{}\"", sender)
                }))
            },
            EmailIntent::CreateFolder(name) => {
                ("create_folder".to_string(), serde_json::json!({
                    "name": name
                }))
            },
            EmailIntent::Help => {
                ("help".to_string(), serde_json::json!({}))
            },
            _ => {
                // Default/unknown mapping
                ("unknown".to_string(), serde_json::json!({
                    "query": entities.search_terms.join(" ")
                }))
            }
        };

        Ok(McpOperation { method, params })
    }
}

impl Default for ExtractedEntities {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            senders: Vec::new(),
            subjects: Vec::new(),
            dates: Vec::new(),
            flags: Vec::new(),
            counts: Vec::new(),
            search_terms: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_pattern_detection() {
        let processor = NlpProcessor::new(ProviderManager::new());

        assert_eq!(
            processor.detect_intent_by_pattern("show me my folders"),
            EmailIntent::ListFolders
        );

        assert_eq!(
            processor.detect_intent_by_pattern("list unread emails"),
            EmailIntent::ShowUnreadEmails
        );

        assert_eq!(
            processor.detect_intent_by_pattern("help"),
            EmailIntent::Help
        );
    }

    #[test]
    fn test_entity_extraction() {
        let processor = NlpProcessor::new(ProviderManager::new());

        let json = serde_json::json!({
            "entities": {
                "folders": ["INBOX", "Sent"],
                "senders": ["john@example.com"],
                "counts": [10]
            }
        });

        let entities = processor.parse_entities_from_json(&json);

        assert_eq!(entities.folders.len(), 2);
        assert_eq!(entities.senders.len(), 1);
        assert_eq!(entities.counts.len(), 1);
    }
}