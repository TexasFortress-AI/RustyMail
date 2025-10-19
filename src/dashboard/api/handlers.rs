// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{web, HttpResponse, Responder};
use actix_web::web::Data;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::Infallible;
use log::{debug, warn, info, error};
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use crate::dashboard::api::models::{ChatbotQuery, ServerConfig};
use crate::dashboard::api::sse::EventType;
use crate::dashboard::services::ai::provider_manager::ProviderConfig;
use actix_web_lab::sse::{self, Sse};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid;
use serde_json;

// Query parameters for client list endpoint
#[derive(Debug, Deserialize)]
pub struct ClientQueryParams {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub filter: Option<String>,
}

// Handler for getting dashboard statistics
pub async fn get_dashboard_stats(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/stats");
    
    let stats = state.metrics_service.get_current_stats().await;
    
    Ok(HttpResponse::Ok().json(stats))
}

// Handler for getting client list
pub async fn get_connected_clients(
    query: web::Query<ClientQueryParams>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/clients with query: {:?}", query);
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let filter = query.filter.as_deref();
    
    if page == 0 {
        return Err(ApiError::BadRequest("Page must be at least 1".to_string()));
    }
    
    if limit == 0 {
        return Err(ApiError::BadRequest("Limit must be at least 1".to_string()));
    }
    
    let clients = state.client_manager.get_clients(page, limit, filter).await;
    
    Ok(HttpResponse::Ok().json(clients))
}

// Handler for getting server configuration
pub async fn get_configuration(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/config");
    
    let config: ServerConfig = state.config_service.get_configuration().await;
    
    Ok(HttpResponse::Ok().json(config))
}

// Handler for chatbot queries
pub async fn query_chatbot(
    state: web::Data<DashboardState>,
    req: web::Json<ChatbotQuery>,
) -> Result<impl Responder, ApiError> {
    info!("Handling POST /api/dashboard/chatbot/query with body: {:?}", req);
    info!("Chatbot query field breakdown:");
    info!("  - query: {}", req.query);
    info!("  - conversation_id: {:?}", req.conversation_id);
    info!("  - provider_override: {:?}", req.provider_override);
    info!("  - model_override: {:?}", req.model_override);
    info!("  - current_folder: {:?}", req.current_folder);
    info!("  - account_id: {:?}", req.account_id);

    let response = state.ai_service.process_query(req.0)
        .await
        .map_err(|e| ApiError::InternalError(format!("AI service error: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

// Handler for listing available MCP tools
/// Get MCP tools in JSON-RPC format for MCP protocol
/// Returns tools with inputSchema following JSON Schema spec
pub fn get_mcp_tools_jsonrpc_format() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "list_folders",
            "description": "List all email folders in the account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "list_folders_hierarchical",
            "description": "List folders with hierarchical structure",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "create_folder",
            "description": "Create a new email folder in the account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_name": {
                        "type": "string",
                        "description": "Name of the folder to create (e.g., INBOX.Archive)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder_name", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "delete_folder",
            "description": "Delete an email folder from the account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder_name": {
                        "type": "string",
                        "description": "Name of the folder to delete (e.g., INBOX.OldEmails)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder_name", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "rename_folder",
            "description": "Rename an email folder in the account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "old_name": {
                        "type": "string",
                        "description": "Current name of the folder (e.g., INBOX.Temp)"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New name for the folder (e.g., INBOX.Projects)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["old_name", "new_name", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "fetch_emails_with_mime",
            "description": "Fetch email content with MIME data",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing the email"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Email UID"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "atomic_move_message",
            "description": "Move a single message to another folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_folder": {
                        "type": "string",
                        "description": "Source folder"
                    },
                    "target_folder": {
                        "type": "string",
                        "description": "Target folder"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Message UID to move"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["source_folder", "target_folder", "uid", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "atomic_batch_move",
            "description": "Move multiple messages to another folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_folder": {
                        "type": "string",
                        "description": "Source folder"
                    },
                    "target_folder": {
                        "type": "string",
                        "description": "Target folder"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["source_folder", "target_folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "mark_as_deleted",
            "description": "Mark messages as deleted",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing messages"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "delete_messages",
            "description": "Permanently delete messages",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing messages"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "undelete_messages",
            "description": "Unmark messages as deleted",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing messages"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "expunge",
            "description": "Expunge deleted messages from folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder to expunge"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "mark_as_read",
            "description": "Mark messages as read",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing messages"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "mark_as_unread",
            "description": "Mark messages as unread",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder containing messages"
                    },
                    "uids": {
                        "type": "string",
                        "description": "Comma-separated list of UIDs"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "uids", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "list_cached_emails",
            "description": "List cached emails from database",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of emails (default: 20)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Pagination offset (default: 0)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "get_email_by_uid",
            "description": "Get full cached email by UID",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Email UID"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["uid", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "get_email_by_index",
            "description": "Get cached email by position index",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "index": {
                        "type": "integer",
                        "description": "Zero-based position index"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["index", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "count_emails_in_folder",
            "description": "Count total emails in cached folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "get_folder_stats",
            "description": "Get statistics about cached folder",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "search_cached_emails",
            "description": "Search within cached emails",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (default: INBOX)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query text"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["query", "account_id"]
            }
        }),
        serde_json::json!({
            "name": "list_accounts",
            "description": "List all configured email accounts",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "set_current_account",
            "description": "Set the current account for email operations",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "Account ID to set as current"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "send_email",
            "description": "Send an email via SMTP",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "array",
                        "description": "REQUIRED. Array of recipient email addresses",
                        "items": {
                            "type": "string"
                        }
                    },
                    "subject": {
                        "type": "string",
                        "description": "REQUIRED. Email subject line"
                    },
                    "body": {
                        "type": "string",
                        "description": "REQUIRED. Plain text email body"
                    },
                    "cc": {
                        "type": "array",
                        "description": "Optional. Array of CC recipient email addresses",
                        "items": {
                            "type": "string"
                        }
                    },
                    "bcc": {
                        "type": "array",
                        "description": "Optional. Array of BCC recipient email addresses",
                        "items": {
                            "type": "string"
                        }
                    },
                    "body_html": {
                        "type": "string",
                        "description": "Optional. HTML email body (multipart with plain text fallback)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "Optional. Email address of the sending account (uses default if not specified)"
                    }
                },
                "required": ["to", "subject", "body"]
            }
        }),
        serde_json::json!({
            "name": "list_email_attachments",
            "description": "List all attachments for a specific email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder containing the email (when using uid)"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Email UID (alternative to message_id)"
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Message ID (alternative to folder+uid)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "download_email_attachments",
            "description": "Download attachments from an email to local directory",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder containing the email (when using uid)"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Email UID (alternative to message_id)"
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Message ID (alternative to folder+uid)"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination directory path (optional)"
                    },
                    "create_zip": {
                        "type": "boolean",
                        "description": "Create ZIP archive instead of individual files (optional, boolean)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "cleanup_attachments",
            "description": "Delete downloaded attachments for a specific email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message_id": {
                        "type": "string",
                        "description": "REQUIRED. The message ID of the email"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["message_id", "account_id"]
            }
        })
    ]
}

pub async fn list_mcp_tools(
    _state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    // List of MCP tools available in the system
    let tools = vec![
        serde_json::json!({
            "name": "list_folders",
            "description": "List all email folders in the account",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "list_folders_hierarchical",
            "description": "List folders with hierarchical structure",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "create_folder",
            "description": "Create a new email folder in the account",
            "parameters": {
                "folder_name": "Name of the folder to create (e.g., INBOX.Archive)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "delete_folder",
            "description": "Delete an email folder from the account",
            "parameters": {
                "folder_name": "Name of the folder to delete (e.g., INBOX.OldEmails)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "rename_folder",
            "description": "Rename an email folder in the account",
            "parameters": {
                "old_name": "Current name of the folder (e.g., INBOX.Temp)",
                "new_name": "New name for the folder (e.g., INBOX.Projects)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "fetch_emails_with_mime",
            "description": "Fetch email content with MIME data",
            "parameters": {
                "folder": "Folder containing the email",
                "uid": "Email UID",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "atomic_move_message",
            "description": "Move a single message to another folder",
            "parameters": {
                "source_folder": "Source folder",
                "target_folder": "Target folder",
                "uid": "Message UID to move",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "atomic_batch_move",
            "description": "Move multiple messages to another folder",
            "parameters": {
                "source_folder": "Source folder",
                "target_folder": "Target folder",
                "uids": "Comma-separated list of UIDs",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "mark_as_deleted",
            "description": "Mark messages as deleted",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "delete_messages",
            "description": "Permanently delete messages",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "undelete_messages",
            "description": "Unmark messages as deleted",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "expunge",
            "description": "Expunge deleted messages from folder",
            "parameters": {
                "folder": "Folder to expunge",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        // Cache-based tools
        serde_json::json!({
            "name": "list_cached_emails",
            "description": "List cached emails from database",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "limit": "Maximum number of emails (default: 20)",
                "offset": "Pagination offset (default: 0)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "get_email_by_uid",
            "description": "Get full cached email by UID",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "uid": "Email UID",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "get_email_by_index",
            "description": "Get cached email by position index",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "index": "Zero-based position index",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "count_emails_in_folder",
            "description": "Count total emails in cached folder",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "get_folder_stats",
            "description": "Get statistics about cached folder",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "search_cached_emails",
            "description": "Search within cached emails",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "query": "Search query text",
                "limit": "Maximum number of results (default: 20)",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        // Account management tools
        serde_json::json!({
            "name": "list_accounts",
            "description": "List all configured email accounts",
            "parameters": {}
        }),
        serde_json::json!({
            "name": "set_current_account",
            "description": "Set the current account for email operations",
            "parameters": {
                "account_id": "Account ID to set as current"
            }
        }),
        // SMTP email sending
        serde_json::json!({
            "name": "send_email",
            "description": "Send an email via SMTP",
            "parameters": {
                "to": "REQUIRED. Array of recipient email addresses",
                "subject": "REQUIRED. Email subject line",
                "body": "REQUIRED. Plain text email body",
                "cc": "Optional. Array of CC recipient email addresses",
                "bcc": "Optional. Array of BCC recipient email addresses",
                "body_html": "Optional. HTML email body (multipart with plain text fallback)",
                "account_id": "Optional. Email address of the sending account (uses default if not specified)"
            }
        }),
        // Attachment management tools
        serde_json::json!({
            "name": "list_email_attachments",
            "description": "List all attachments for a specific email",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)",
                "folder": "Folder containing the email (when using uid)",
                "uid": "Email UID (alternative to message_id)",
                "message_id": "Message ID (alternative to folder+uid)"
            }
        }),
        serde_json::json!({
            "name": "download_email_attachments",
            "description": "Download attachments from an email to local directory",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)",
                "folder": "Folder containing the email (when using uid)",
                "uid": "Email UID (alternative to message_id)",
                "message_id": "Message ID (alternative to folder+uid)",
                "destination": "Destination directory path (optional)",
                "create_zip": "Create ZIP archive instead of individual files (optional, boolean)"
            }
        }),
        serde_json::json!({
            "name": "cleanup_attachments",
            "description": "Delete downloaded attachments for a specific email",
            "parameters": {
                "message_id": "REQUIRED. The message ID of the email",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "mark_as_read",
            "description": "Mark messages as read (adds \\Seen flag)",
            "parameters": {
                "folder": "REQUIRED. Folder containing messages",
                "uids": "REQUIRED. Array of message UIDs to mark as read",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        }),
        serde_json::json!({
            "name": "mark_as_unread",
            "description": "Mark messages as unread (removes \\Seen flag)",
            "parameters": {
                "folder": "REQUIRED. Folder containing messages",
                "uids": "REQUIRED. Array of message UIDs to mark as unread",
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)"
            }
        })
    ];

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tools": tools
    })))
}

/// Helper function to get account_id from request parameters
/// REQUIRES account_id to be provided as an email address
/// Returns the email address directly (no UUID lookup)
async fn get_account_id_to_use(
    params: &serde_json::Value,
    _state: &web::Data<DashboardState>,
) -> Result<String, ApiError> {
    // account_id is REQUIRED and must be an email address
    if let Some(account_id) = params.get("account_id").and_then(|v| v.as_str()) {
        return Ok(account_id.to_string());
    }

    // If account_id not provided, return error
    Err(ApiError::BadRequest(
        "account_id parameter is required and must be an email address (e.g., user@example.com)".to_string()
    ))
}

/// Validate that an account exists by checking if it can be retrieved
/// Returns the account_id (email address) if the account is found
async fn validate_account_exists(
    account_id: &str,
    state: &DashboardState,
) -> Result<String, ApiError> {
    // Verify the account exists by looking it up
    let account_service = state.account_service.lock().await;
    let _account = account_service.get_account(account_id).await
        .map_err(|e| ApiError::NotFound(format!("Account not found: {}", e)))?;
    drop(account_service); // Release lock

    // Return the account_id
    Ok(account_id.to_string())
}

/// Inner function that executes MCP tools and returns raw JSON result
/// Can be called from both HTTP handler and MCP protocol handler
pub async fn execute_mcp_tool_inner(
    state: &DashboardState,
    tool_name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    debug!("Executing MCP tool: {} with params: {:?}", tool_name, params);

    // Get the email service from the state
    let email_service = state.email_service.clone();

    // Create a temporary web::Data wrapper for helper functions that need it
    let state_data = web::Data::new(state.clone());

    // Execute the appropriate tool
    let result = match tool_name {
        "list_folders" => {
            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            match email_service.list_folders_for_account(&account_id).await {
                Ok(folders) => {
                    serde_json::json!({
                        "success": true,
                        "data": folders,
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to list folders: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "list_folders_hierarchical" => {
            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            // For now, just use regular list_folders since hierarchical is not implemented
            match email_service.list_folders_for_account(&account_id).await {
                Ok(folders) => {
                    serde_json::json!({
                        "success": true,
                        "data": folders,
                        "tool": tool_name,
                        "note": "Using flat list - hierarchical not yet implemented"
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to list folders: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "create_folder" => {
            let folder_name = match params.get("folder_name").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder_name' parameter",
                    "tool": tool_name
                })
            };

            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            match email_service.create_folder_for_account(folder_name, &account_id).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "folder_name": folder_name,
                            "account_id": account_id
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to create folder: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "delete_folder" => {
            let folder_name = match params.get("folder_name").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder_name' parameter",
                    "tool": tool_name
                })
            };

            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            match email_service.delete_folder_for_account(folder_name, &account_id).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "folder_name": folder_name,
                            "account_id": account_id
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to delete folder: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "rename_folder" => {
            let old_name = match params.get("old_name").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'old_name' parameter",
                    "tool": tool_name
                })
            };
            let new_name = match params.get("new_name").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'new_name' parameter",
                    "tool": tool_name
                })
            };

            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            match email_service.rename_folder_for_account(old_name, new_name, &account_id).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "old_name": old_name,
                            "new_name": new_name,
                            "account_id": account_id
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to rename folder: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "fetch_emails_with_mime" => {
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };
            let uid = match params.get("uid").and_then(|v| v.as_u64()) {
                Some(u) => u as u32,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uid' parameter",
                    "tool": tool_name
                })
            };

            // Get account ID from request or use default
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            // fetch_emails expects an array of UIDs
            match email_service.fetch_emails_for_account(folder, &[uid], &account_id).await {
                Ok(emails) => {
                    // Return just the first email if found
                    let email_data = emails.into_iter().next();
                    serde_json::json!({
                        "success": true,
                        "data": email_data,
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to fetch email: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        // Cache-based tools
        "list_cached_emails" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let limit = params.get("limit")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(20);
            let offset = params.get("offset")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(0);
            let preview_mode = params.get("preview_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);  // Default to preview mode for token efficiency

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };
                    match state.cache_service.get_cached_emails_for_account(folder, &account_email, limit, offset, preview_mode).await {
                        Ok(emails) => {
                            serde_json::json!({
                                "success": true,
                                "data": emails,
                                "folder": folder,
                                "count": emails.len(),
                                "tool": tool_name
                            })
                        }
                        Err(e) => {
                            serde_json::json!({
                                "success": false,
                                "error": format!("Failed to get cached emails: {}", e),
                                "tool": tool_name
                            })
                        }
                    }
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to determine account: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "get_email_by_uid" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let uid = params.get("uid")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };
                    if let Some(uid) = uid {
                        match state.cache_service.get_email_by_uid_for_account(folder, uid, &account_email).await {
                            Ok(Some(email)) => {
                                serde_json::json!({
                                    "success": true,
                                    "data": email,
                                    "tool": tool_name
                                })
                            }
                            Ok(None) => {
                                serde_json::json!({
                                    "success": false,
                                    "error": format!("Email with UID {} not found in {}", uid, folder),
                                    "tool": tool_name
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "success": false,
                                    "error": format!("Failed to get email by UID: {}", e),
                                    "tool": tool_name
                                })
                            }
                        }
                    } else {
                        serde_json::json!({
                            "success": false,
                            "error": "UID parameter is required",
                            "tool": tool_name
                        })
                    }
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to determine account: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "get_email_by_index" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let index = params.get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };
                    if let Some(index) = index {
                        // Get emails sorted by date DESC, then select by index
                        // Dashboard UI needs full content for display
                        match state.cache_service.get_cached_emails_for_account(folder, &account_email, index + 1, index, false).await {
                    Ok(emails) if !emails.is_empty() => {
                        serde_json::json!({
                            "success": true,
                            "data": emails[0],
                            "tool": tool_name
                        })
                    }
                    Ok(_) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("No email at index {} in {}", index, folder),
                            "tool": tool_name
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to get email by index: {}", e),
                            "tool": tool_name
                        })
                    }
                        }
                    } else {
                        serde_json::json!({
                            "success": false,
                            "error": "index parameter is required",
                            "tool": tool_name
                        })
                    }
                }
                Err(e) => {
                serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
                }
            }
        }
        "count_emails_in_folder" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };

            match state.cache_service.count_emails_in_folder_for_account(folder, &account_email).await {
                Ok(count) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "count": count,
                            "folder": folder
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to count emails: {}", e),
                        "tool": tool_name
                    })
                }
            }
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to determine account: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "get_folder_stats" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };

            match state.cache_service.get_folder_stats_for_account(folder, &account_email).await {
                Ok(stats) => {
                    serde_json::json!({
                        "success": true,
                        "data": stats,
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to get folder stats: {}", e),
                        "tool": tool_name
                    })
                }
            }
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to determine account: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "search_cached_emails" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let query = params.get("query")
                .and_then(|v| v.as_str());
            let limit = params.get("limit")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(20);

            // Get account ID from request or use default
            match get_account_id_to_use(&params, &state_data).await {
                Ok(account_id) => {
                    let account_email = match validate_account_exists(&account_id, &state).await {
                        Ok(id) => id,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "error": format!("Failed to lookup account: {}", e)
                            });
                        }
                    };

            if let Some(query) = query {
                match state.cache_service.search_cached_emails_for_account(folder, query, limit, &account_email).await {
                    Ok(emails) => {
                        serde_json::json!({
                            "success": true,
                            "data": emails,
                            "query": query,
                            "folder": folder,
                            "count": emails.len(),
                            "tool": tool_name
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to search emails: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            } else {
                serde_json::json!({
                    "success": false,
                    "error": "query parameter is required",
                    "tool": tool_name
                })
            }
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to determine account: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "atomic_move_message" => {
            let uid = match params.get("uid").and_then(|v| v.as_u64()) {
                Some(u) => u as u32,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uid' parameter",
                    "tool": tool_name
                })
            };
            let from_folder = match params.get("from_folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'from_folder' parameter",
                    "tool": tool_name
                })
            };
            let to_folder = match params.get("to_folder").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'to_folder' parameter",
                    "tool": tool_name
                })
            };

            match email_service.atomic_move_message(uid, from_folder, to_folder).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uid": uid,
                            "from_folder": from_folder,
                            "to_folder": to_folder
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to move message: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "atomic_batch_move" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let from_folder = match params.get("from_folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'from_folder' parameter",
                    "tool": tool_name
                })
            };
            let to_folder = match params.get("to_folder").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'to_folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.atomic_batch_move(&uids, from_folder, to_folder).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "from_folder": from_folder,
                            "to_folder": to_folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to batch move messages: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "mark_as_read" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.mark_as_read(folder, &uids).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "folder": folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to mark messages as read: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "mark_as_unread" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.mark_as_unread(folder, &uids).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "folder": folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to mark messages as unread: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "mark_as_deleted" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.mark_as_deleted(folder, &uids).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "folder": folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to mark messages as deleted: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "delete_messages" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.delete_messages(folder, &uids).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "folder": folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to delete messages: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "undelete_messages" => {
            let uids = match params.get("uids").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect::<Vec<u32>>(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'uids' parameter",
                    "tool": tool_name
                })
            };
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            if uids.is_empty() {
                return serde_json::json!({
                    "success": false,
                    "error": "'uids' parameter cannot be empty",
                    "tool": tool_name
                });
            }

            match email_service.undelete_messages(folder, &uids).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "uids": uids,
                            "folder": folder,
                            "count": uids.len()
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to undelete messages: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "expunge" => {
            let folder = match params.get("folder").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'folder' parameter",
                    "tool": tool_name
                })
            };

            match email_service.expunge(folder).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "folder": folder
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to expunge folder: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "list_accounts" => {
            // List all configured email accounts
            let account_service = state.account_service.lock().await;
            match account_service.list_accounts().await {
                Ok(accounts) => {
                    serde_json::json!({
                        "success": true,
                        "data": accounts,
                        "count": accounts.len(),
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to list accounts: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "set_current_account" => {
            // Set the current account context
            let account_id = match params.get("account_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing 'account_id' parameter",
                    "tool": tool_name
                })
            };

            // Validate that the account exists
            let account_service = state.account_service.lock().await;
            match account_service.get_account(account_id).await {
                Ok(account) => {
                    // Account exists - in a real implementation, we would store this in session state
                    // For now, just return success with the account info
                    serde_json::json!({
                        "success": true,
                        "message": format!("Current account set to: {}", account_id),
                        "data": {
                            "account_id": account_id,
                            "display_name": account.display_name,
                            "email_address": account.email_address
                        },
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Account not found: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "send_email" => {
            use crate::dashboard::services::{SendEmailRequest};

            // Helper function to parse email addresses (handles string or array)
            let parse_emails = |key: &str, required: bool| -> Result<Vec<String>, String> {
                match params.get(key) {
                    Some(val) => {
                        if val.is_string() {
                            let s = val.as_str().unwrap_or("");
                            if s.is_empty() {
                                if required {
                                    return Err(format!("{} cannot be empty", key));
                                }
                                Ok(vec![])
                            } else {
                                Ok(vec![s.to_string()])
                            }
                        } else if val.is_array() {
                            let emails = val.as_array().unwrap()
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<String>>();
                            if required && emails.is_empty() {
                                return Err(format!("{} cannot be empty", key));
                            }
                            Ok(emails)
                        } else {
                            Err(format!("{} must be a string or array", key))
                        }
                    }
                    None => {
                        if required {
                            Err(format!("{} is required", key))
                        } else {
                            Ok(vec![])
                        }
                    }
                }
            };

            // Parse required fields
            let to = match parse_emails("to", true) {
                Ok(emails) => emails,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": e,
                    "tool": tool_name
                })
            };

            let subject = params.get("subject")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or("subject is required")
                .map(String::from);

            let subject = match subject {
                Ok(s) => s,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": e,
                    "tool": tool_name
                })
            };

            let body = params.get("body")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or("body is required")
                .map(String::from);

            let body = match body {
                Ok(b) => b,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": e,
                    "tool": tool_name
                })
            };

            // Parse optional fields
            let cc = parse_emails("cc", false).ok()
                .filter(|v| !v.is_empty());

            let bcc = parse_emails("bcc", false).ok()
                .filter(|v| !v.is_empty());

            let body_html = params.get("body_html")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            // Build the send request
            let send_request = SendEmailRequest {
                to,
                cc,
                bcc,
                subject,
                body,
                body_html,
            };

            // Get account email - use account_id from params or default
            let account_email = if let Some(account_id_val) = params.get("account_id") {
                if let Some(account_id_str) = account_id_val.as_str() {
                    account_id_str.to_string()
                } else {
                    // Get default account if account_id is not a string
                    let account_service = state.account_service.lock().await;
                    match account_service.get_default_account().await {
                        Ok(Some(account)) => account.email_address,
                        Ok(None) => return serde_json::json!({
                            "success": false,
                            "error": "No default account configured",
                            "tool": tool_name
                        }),
                        Err(e) => return serde_json::json!({
                            "success": false,
                            "error": format!("Failed to get default account: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            } else {
                // No account_id provided - use default
                let account_service = state.account_service.lock().await;
                match account_service.get_default_account().await {
                    Ok(Some(account)) => account.email_address,
                    Ok(None) => return serde_json::json!({
                        "success": false,
                        "error": "No default account configured",
                        "tool": tool_name
                    }),
                    Err(e) => return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to get default account: {}", e),
                        "tool": tool_name
                    })
                }
            };

            // Send the email using SMTP service
            match state.smtp_service.send_email(&account_email, send_request).await {
                Ok(response) => {
                    serde_json::json!({
                        "success": response.success,
                        "message": response.message,
                        "message_id": response.message_id,
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to send email: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "list_email_attachments" => {
            use crate::dashboard::services::attachment_storage;

            // Get account ID
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            // Get database pool
            let db_pool = match state.cache_service.db_pool.as_ref() {
                Some(pool) => pool,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Database not available",
                    "tool": tool_name
                })
            };

            // Determine message_id - either directly provided or resolve from folder+uid
            // Also track folder and uid for potential IMAP fetch
            let (message_id, folder_opt, uid_opt) = if let Some(msg_id) = params.get("message_id").and_then(|v| v.as_str()) {
                // message_id provided directly - no folder/uid available
                (msg_id.to_string(), None, None)
            } else {
                // Resolve from folder + uid
                let folder = match params.get("folder").and_then(|v| v.as_str()) {
                    Some(f) => f,
                    None => return serde_json::json!({
                        "success": false,
                        "error": "folder parameter required when message_id not provided",
                        "tool": tool_name
                    })
                };

                let uid = match params.get("uid").and_then(|v| v.as_u64()) {
                    Some(u) => u as u32,
                    None => return serde_json::json!({
                        "success": false,
                        "error": "uid parameter required when message_id not provided",
                        "tool": tool_name
                    })
                };

                // Fetch email to get message_id
                let msg_id = match email_service.fetch_emails_for_account(folder, &[uid], &account_id).await {
                    Ok(mut emails) if !emails.is_empty() => {
                        let email = emails.remove(0);
                        attachment_storage::ensure_message_id(&email, &account_id)
                    }
                    Ok(_) => return serde_json::json!({
                        "success": false,
                        "error": format!("Email with UID {} not found", uid),
                        "tool": tool_name
                    }),
                    Err(e) => return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to fetch email: {}", e),
                        "tool": tool_name
                    })
                };

                (msg_id, Some(folder.to_string()), Some(uid))
            };

            // Get attachments metadata from database
            let attachments = match attachment_storage::get_attachments_metadata(db_pool, &account_id, &message_id).await {
                Ok(atts) => atts,
                Err(e) => {
                    return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to get attachments: {}", e),
                        "tool": tool_name
                    })
                }
            };

            // If no attachments found in database and we have folder+uid, fetch from IMAP
            if attachments.is_empty() && folder_opt.is_some() && uid_opt.is_some() {
                let folder = folder_opt.unwrap();
                let uid = uid_opt.unwrap();

                debug!("No attachments in database for message_id {}. Fetching from IMAP...", message_id);

                // Fetch email with attachments from IMAP (this will save them to DB)
                match email_service.fetch_email_with_attachments(&folder, uid, &account_id).await {
                    Ok((_, attachment_infos)) => {
                        // Return the newly fetched attachments
                        serde_json::json!({
                            "success": true,
                            "message_id": message_id,
                            "account_id": account_id,
                            "attachments": attachment_infos,
                            "count": attachment_infos.len(),
                            "fetched_from_imap": true,
                            "tool": tool_name
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to fetch attachments from IMAP: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            } else {
                // Return attachments from database cache
                serde_json::json!({
                    "success": true,
                    "message_id": message_id,
                    "account_id": account_id,
                    "attachments": attachments,
                    "count": attachments.len(),
                    "tool": tool_name
                })
            }
        }
        "download_email_attachments" => {
            use crate::dashboard::services::attachment_storage;

            // Get account ID
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            // Get database pool
            let db_pool = match state.cache_service.db_pool.as_ref() {
                Some(pool) => pool,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Database not available",
                    "tool": tool_name
                })
            };

            // Determine message_id - either directly provided or resolve from folder+uid
            // Also track folder and uid for potential IMAP fetch
            let (message_id, folder_opt, uid_opt) = if let Some(msg_id) = params.get("message_id").and_then(|v| v.as_str()) {
                // message_id provided directly - no folder/uid available
                (msg_id.to_string(), None, None)
            } else {
                // Resolve from folder + uid
                let folder = match params.get("folder").and_then(|v| v.as_str()) {
                    Some(f) => f,
                    None => return serde_json::json!({
                        "success": false,
                        "error": "folder parameter required when message_id not provided",
                        "tool": tool_name
                    })
                };

                let uid = match params.get("uid").and_then(|v| v.as_u64()) {
                    Some(u) => u as u32,
                    None => return serde_json::json!({
                        "success": false,
                        "error": "uid parameter required when message_id not provided",
                        "tool": tool_name
                    })
                };

                // Fetch email to get message_id
                let msg_id = match email_service.fetch_emails_for_account(folder, &[uid], &account_id).await {
                    Ok(mut emails) if !emails.is_empty() => {
                        let email = emails.remove(0);
                        attachment_storage::ensure_message_id(&email, &account_id)
                    }
                    Ok(_) => return serde_json::json!({
                        "success": false,
                        "error": format!("Email with UID {} not found", uid),
                        "tool": tool_name
                    }),
                    Err(e) => return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to fetch email: {}", e),
                        "tool": tool_name
                    })
                };

                (msg_id, Some(folder.to_string()), Some(uid))
            };

            // Check if attachments exist in database, fetch from IMAP if not
            let mut attachments = match attachment_storage::get_attachments_metadata(db_pool, &account_id, &message_id).await {
                Ok(atts) => atts,
                Err(e) => {
                    return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to get attachments: {}", e),
                        "tool": tool_name
                    })
                }
            };

            // If no attachments found in database and we have folder+uid, fetch from IMAP
            if attachments.is_empty() && folder_opt.is_some() && uid_opt.is_some() {
                let folder = folder_opt.as_ref().unwrap();
                let uid = uid_opt.unwrap();

                debug!("No attachments in database for message_id {}. Fetching from IMAP...", message_id);

                // Fetch email with attachments from IMAP (this will save them to DB)
                match email_service.fetch_email_with_attachments(folder, uid, &account_id).await {
                    Ok((_, attachment_infos)) => {
                        // Attachments now saved to database
                        debug!("Successfully fetched and saved {} attachments from IMAP", attachment_infos.len());

                        // Re-query database to get the saved attachments
                        attachments = match attachment_storage::get_attachments_metadata(db_pool, &account_id, &message_id).await {
                            Ok(atts) => atts,
                            Err(e) => {
                                return serde_json::json!({
                                    "success": false,
                                    "error": format!("Failed to get attachments after IMAP fetch: {}", e),
                                    "tool": tool_name
                                })
                            }
                        };
                    }
                    Err(e) => {
                        return serde_json::json!({
                            "success": false,
                            "error": format!("Failed to fetch attachments from IMAP: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            }

            // Check if user wants ZIP archive
            let create_zip = params.get("create_zip")
                .and_then(|v| v.as_bool())
                .unwrap_or(true); // Default to ZIP for convenience

            if create_zip {
                // Create ZIP archive
                let temp_dir = std::env::temp_dir();
                let sanitized_id = attachment_storage::sanitize_message_id(&message_id);
                let zip_path = temp_dir.join(format!("rustymail_attachments_{}.zip", sanitized_id));

                match attachment_storage::create_zip_archive(db_pool, &account_id, &message_id, &zip_path).await {
                    Ok(result_path) => {
                        serde_json::json!({
                            "success": true,
                            "message": "ZIP archive created",
                            "zip_path": result_path.to_string_lossy(),
                            "message_id": message_id,
                            "account_id": account_id,
                            "tool": tool_name
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to create ZIP: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            } else {
                // Just list attachment paths without creating ZIP
                match attachment_storage::get_attachments_metadata(db_pool, &account_id, &message_id).await {
                    Ok(attachments) => {
                        let paths: Vec<String> = attachments.iter()
                            .map(|a| a.storage_path.clone())
                            .collect();

                        serde_json::json!({
                            "success": true,
                            "message": "Attachment paths retrieved",
                            "paths": paths,
                            "attachments": attachments,
                            "message_id": message_id,
                            "account_id": account_id,
                            "tool": tool_name
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to get attachments: {}", e),
                            "tool": tool_name
                        })
                    }
                }
            }
        }
        "cleanup_attachments" => {
            use crate::dashboard::services::attachment_storage;

            // Get required parameters
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let message_id = match params.get("message_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return serde_json::json!({
                    "success": false,
                    "error": "message_id parameter is required",
                    "tool": tool_name
                })
            };

            // Get database pool
            let db_pool = match state.cache_service.db_pool.as_ref() {
                Some(pool) => pool,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Database not available",
                    "tool": tool_name
                })
            };

            // Delete attachments
            match attachment_storage::delete_attachments_for_email(db_pool, message_id, &account_id).await {
                Ok(_) => {
                    serde_json::json!({
                        "success": true,
                        "message": format!("Attachments deleted for message {}", message_id),
                        "message_id": message_id,
                        "account_id": account_id,
                        "tool": tool_name
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "success": false,
                        "error": format!("Failed to delete attachments: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        _ => {
            // For other tools not yet implemented
            serde_json::json!({
                "success": false,
                "message": format!("Tool '{}' execution not yet implemented", tool_name),
                "tool": tool_name
            })
        }
    };

    result
}

// HTTP Handler for executing MCP tools - wraps execute_mcp_tool_inner
pub async fn execute_mcp_tool(
    state: web::Data<DashboardState>,
    req: web::Json<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    let tool_name = req.get("tool")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Missing tool name".to_string()))?;

    let params = req.get("parameters")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Call the inner function to get the result
    let result = execute_mcp_tool_inner(state.get_ref(), tool_name, params).await;

    Ok(HttpResponse::Ok().json(result))
}

// Handler for streaming chatbot responses via SSE
pub async fn stream_chatbot(
    state: web::Data<DashboardState>,
    req: web::Json<ChatbotQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<sse::Event, Infallible>>>, ApiError> {
    debug!("Handling POST /api/dashboard/chatbot/stream with body: {:?}", req);

    let (tx, rx) = mpsc::channel(100);
    let ai_service = state.ai_service.clone();
    let query = req.into_inner();

    // Spawn task to process query and stream response
    tokio::spawn(async move {
        // First send a "start" event
        let start_event = sse::Data::new(serde_json::json!({
            "type": "start",
            "conversation_id": query.conversation_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        }).to_string())
            .event("chatbot");

        if tx.send(Ok(sse::Event::Data(start_event))).await.is_err() {
            return;
        }

        // Process the query
        match ai_service.process_query(query).await {
            Ok(response) => {
                // For now, send the full response at once
                // TODO: Implement actual token-by-token streaming when provider supports it
                let content_event = sse::Data::new(serde_json::json!({
                    "type": "content",
                    "text": response.text,
                    "conversation_id": response.conversation_id,
                    "email_data": response.email_data,
                    "followup_suggestions": response.followup_suggestions
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(content_event))).await;

                // Send completion event
                let complete_event = sse::Data::new(serde_json::json!({
                    "type": "complete"
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(complete_event))).await;
            }
            Err(e) => {
                // Send error event
                let error_event = sse::Data::new(serde_json::json!({
                    "type": "error",
                    "error": format!("AI service error: {}", e)
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(error_event))).await;
            }
        }
    });

    // Convert receiver to stream
    let stream = ReceiverStream::new(rx);

    Ok(Sse::from_stream(stream))
}

// Request/response structures for subscription management
#[derive(Debug, Deserialize)]
pub struct SubscriptionRequest {
    pub event_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub client_id: String,
    pub subscriptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub event_type: String,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub event_type: String,
}

// Path parameters for subscription endpoints
#[derive(Debug, Deserialize)]
pub struct ClientIdPath {
    pub client_id: String,
}

// Handler for getting client subscriptions
pub async fn get_client_subscriptions(
    path: web::Path<ClientIdPath>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/clients/{}/subscriptions", path.client_id);

    match state.sse_manager.get_client_subscriptions(&path.client_id).await {
        Some(subscriptions) => {
            let subscription_strings: Vec<String> = subscriptions
                .iter()
                .map(|et| et.to_string().to_string())
                .collect();

            let response = SubscriptionResponse {
                client_id: path.client_id.clone(),
                subscriptions: subscription_strings,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        None => Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for updating client subscriptions (PUT - replaces all)
pub async fn update_client_subscriptions(
    path: web::Path<ClientIdPath>,
    req: web::Json<SubscriptionRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling PUT /api/dashboard/clients/{}/subscriptions", path.client_id);

    // Convert string event types to EventType enum
    let mut event_types = HashSet::new();
    for event_str in &req.event_types {
        match EventType::from_string(event_str) {
            Some(event_type) => {
                event_types.insert(event_type);
            }
            None => {
                return Err(ApiError::BadRequest(format!("Invalid event type: {}", event_str)));
            }
        }
    }

    // Update subscriptions
    let success = state.sse_manager
        .update_client_subscriptions(&path.client_id, event_types.clone())
        .await;

    if success {
        let subscription_strings: Vec<String> = event_types
            .iter()
            .map(|et| et.to_string().to_string())
            .collect();

        let response = SubscriptionResponse {
            client_id: path.client_id.clone(),
            subscriptions: subscription_strings,
        };

        Ok(HttpResponse::Ok().json(response))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for subscribing to a single event type (POST)
pub async fn subscribe_to_event(
    path: web::Path<ClientIdPath>,
    req: web::Json<SubscribeRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/clients/{}/subscribe", path.client_id);

    // Convert string to EventType
    let event_type = EventType::from_string(&req.event_type)
        .ok_or_else(|| ApiError::BadRequest(format!("Invalid event type: {}", req.event_type)))?;

    // Subscribe client
    let success = state.sse_manager
        .subscribe_client_to_event(&path.client_id, event_type)
        .await;

    if success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Client {} subscribed to {}", path.client_id, req.event_type)
        })))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for unsubscribing from a single event type (POST)
pub async fn unsubscribe_from_event(
    path: web::Path<ClientIdPath>,
    req: web::Json<UnsubscribeRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/clients/{}/unsubscribe", path.client_id);

    // Convert string to EventType
    let event_type = EventType::from_string(&req.event_type)
        .ok_or_else(|| ApiError::BadRequest(format!("Invalid event type: {}", req.event_type)))?;

    // Unsubscribe client
    let success = state.sse_manager
        .unsubscribe_client_from_event(&path.client_id, &event_type)
        .await;

    if success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Client {} unsubscribed from {}", path.client_id, req.event_type)
        })))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for listing available event types
pub async fn get_available_event_types() -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/events/types");

    let event_types = vec![
        EventType::Welcome.to_string(),
        EventType::StatsUpdate.to_string(),
        EventType::ClientConnected.to_string(),
        EventType::ClientDisconnected.to_string(),
        EventType::SystemAlert.to_string(),
        EventType::ConfigurationUpdated.to_string(),
        EventType::DashboardEvent.to_string(),
    ];

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "available_event_types": event_types,
        "description": {
            "welcome": "Welcome message sent when client connects",
            "stats_update": "Real-time dashboard statistics updates",
            "client_connected": "Notifications when new clients connect",
            "client_disconnected": "Notifications when clients disconnect",
            "system_alert": "System alerts and warnings",
            "configuration_updated": "Configuration change notifications",
            "dashboard_event": "Generic dashboard events"
        }
    })))
}

// AI Provider Management Handlers

#[derive(Debug, Deserialize)]
pub struct SetProviderRequest {
    pub provider_name: String,
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub current_provider: Option<String>,
    pub available_providers: Vec<ProviderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SetModelRequest {
    pub model_name: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub current_model: Option<String>,
    pub available_models: Vec<String>,
    pub provider: Option<String>,
}

// Handler for getting AI provider status and list
pub async fn get_ai_providers(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/providers");

    let providers = state.ai_service.list_providers().await;
    let current_provider = state.ai_service.get_current_provider_name().await;

    let response = ProvidersResponse {
        current_provider,
        available_providers: providers,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Handler for setting the current AI provider
pub async fn set_ai_provider(
    req: web::Json<SetProviderRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/providers/set with provider: {}", req.provider_name);

    // Set the current provider
    state.ai_service
        .set_current_provider(req.provider_name.clone())
        .await
        .map_err(|e| ApiError::BadRequest(e))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Successfully switched to provider: {}", req.provider_name),
        "current_provider": req.provider_name
    })))
}

// Handler for getting available models for current AI provider
pub async fn get_ai_models(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/models");

    let current_provider = state.ai_service.get_current_provider_name().await;
    let providers = state.ai_service.list_providers().await;

    // Get current model from the current provider
    let current_model = if let Some(ref provider_name) = current_provider {
        providers.iter()
            .find(|p| p.name == *provider_name)
            .map(|p| p.model.clone())
    } else {
        None
    };

    // Dynamically fetch available models from the current provider
    let available_models = if let Some(provider_name) = current_provider.as_deref() {
        match state.ai_service.get_available_models().await {
            Ok(models) => models,
            Err(e) => {
                warn!("Failed to fetch models from provider {}: {:?}", provider_name, e);
                // Fallback to empty list if API call fails
                vec![]
            }
        }
    } else {
        vec![]
    };

    let response = ModelsResponse {
        current_model,
        available_models,
        provider: current_provider,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Handler for setting the model for current AI provider
pub async fn set_ai_model(
    req: web::Json<SetModelRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/models/set with model: {}", req.model_name);

    let current_provider = state.ai_service.get_current_provider_name().await;

    if let Some(provider_name) = current_provider {
        // Get current provider config
        let providers = state.ai_service.list_providers().await;
        if let Some(current_config) = providers.iter().find(|p| p.name == provider_name) {
            // Create updated config with new model
            let mut new_config = current_config.clone();
            new_config.model = req.model_name.clone();

            // Update the provider config
            state.ai_service
                .update_provider_config(&provider_name, new_config)
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to update model: {}", e)))?;

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": format!("Successfully set model to: {}", req.model_name),
                "model": req.model_name,
                "provider": provider_name
            })))
        } else {
            Err(ApiError::BadRequest("Current provider configuration not found".to_string()))
        }
    } else {
        Err(ApiError::BadRequest("No current provider set".to_string()))
    }
}

/// Trigger a full email sync
pub async fn trigger_email_sync(
    state: Data<DashboardState>,
    query: web::Query<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    // Get account ID from query parameters or use default
    let account_id = match get_account_id_to_use(&query.0, &state).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to determine account for sync: {}", e);
            return Err(e);
        }
    };

    info!("Triggering full email sync for all folders for account: {}", account_id);

    // Clone the sync service Arc and account_id to move into the async task
    let sync_service = state.sync_service.clone();
    let account_id_for_task = account_id.clone();

    // Spawn the sync task in the background
    tokio::spawn(async move {
        // Use sync_all_folders which dynamically fetches folder list from IMAP
        match sync_service.sync_all_folders(&account_id_for_task).await {
            Ok(()) => info!("All folder syncs completed successfully for account {}", account_id_for_task),
            Err(e) => error!("Sync failed for account {}: {}", account_id_for_task, e),
        }
    });

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Email sync started in background for all folders for account {}", account_id),
        "status": "syncing",
        "account_id": account_id
    })))
}

/// Get the current sync status
pub async fn get_sync_status(
    state: Data<DashboardState>,
    query: web::Query<EmailQueryParams>,
) -> Result<impl Responder, ApiError> {
    // Get account ID from query parameters or use default
    let account_id = match query.account_id.as_ref() {
        Some(id) => id.clone(),
        None => {
            // Get default account if no account_id provided
            let account_service = state.account_service.lock().await;
            match account_service.get_default_account().await {
                Ok(Some(account)) => account.email_address,
                Ok(None) => return Err(ApiError::NotFound("No default account configured".to_string())),
                Err(e) => return Err(ApiError::InternalError(format!("Failed to get default account: {}", e))),
            }
        }
    };

    let account_email = match validate_account_exists(&account_id, &state).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to lookup database account ID: {}", e);
            return Err(e);
        }
    };

    let folder = query.folder.as_deref().unwrap_or("INBOX");

    // Get sync state for folder
    match state.cache_service.get_sync_state(folder, &account_email).await {
        Ok(Some(sync_state)) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "folder": "INBOX",
                "status": format!("{:?}", sync_state.sync_status),
                "last_uid_synced": sync_state.last_uid_synced,
                "last_full_sync": sync_state.last_full_sync,
                "last_incremental_sync": sync_state.last_incremental_sync,
                "error_message": sync_state.error_message
            })))
        }
        Ok(None) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "folder": "INBOX",
                "status": "never_synced",
                "last_uid_synced": null,
                "last_full_sync": null,
                "last_incremental_sync": null,
                "error_message": null
            })))
        }
        Err(e) => {
            error!("Failed to get sync status: {}", e);
            Err(ApiError::InternalError(format!("Failed to get sync status: {}", e)))
        }
    }
}

/// Get cached emails from the database
#[derive(serde::Deserialize)]
pub struct EmailQueryParams {
    folder: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    account_id: Option<String>,
}

pub async fn list_folders(
    state: Data<DashboardState>,
    query: web::Query<EmailQueryParams>,
) -> Result<impl Responder, ApiError> {
    // Get account ID from query parameters or use default
    let account_id = match query.account_id.as_ref() {
        Some(id) => id.clone(),
        None => {
            // Get default account if no account_id provided
            let account_service = state.account_service.lock().await;
            match account_service.get_default_account().await {
                Ok(Some(account)) => account.email_address,
                Ok(None) => return Err(ApiError::NotFound("No default account configured".to_string())),
                Err(e) => return Err(ApiError::InternalError(format!("Failed to get default account: {}", e))),
            }
        }
    };

    info!("Listing folders for account: {}", account_id);

    // List folders for the account
    match state.email_service.list_folders_for_account(&account_id).await {
        Ok(folders) => {
            info!("Found {} folders for account {}", folders.len(), account_id);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "account_id": account_id,
                "folders": folders
            })))
        }
        Err(e) => {
            error!("Failed to list folders for account {}: {}", account_id, e);
            Err(ApiError::InternalError(format!("Failed to list folders: {}", e)))
        }
    }
}

pub async fn get_cached_emails(
    state: Data<DashboardState>,
    query: web::Query<EmailQueryParams>,
) -> Result<impl Responder, ApiError> {
    let folder = query.folder.as_deref().unwrap_or("INBOX");
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // Get account ID from query parameters or use default
    let account_id = match query.account_id.as_ref() {
        Some(id) => id.clone(),
        None => {
            // Get default account if no account_id provided
            let account_service = state.account_service.lock().await;
            match account_service.get_default_account().await {
                Ok(Some(account)) => account.email_address,
                Ok(None) => return Err(ApiError::NotFound("No default account configured".to_string())),
                Err(e) => return Err(ApiError::InternalError(format!("Failed to get default account: {}", e))),
            }
        }
    };

    let account_email = match validate_account_exists(&account_id, &state).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to lookup database account ID: {}", e);
            return Err(e);
        }
    };

    info!("Getting cached emails for folder: {}, account: {}, limit: {}, offset: {}",
          folder, account_id, limit, offset);

    // Dashboard UI needs full content for display
    match state.cache_service.get_cached_emails_for_account(folder, &account_email, limit, offset, false).await {
        Ok(emails) => {
            // Get total count for this folder and account
            let total_count = state.cache_service.count_emails_in_folder_for_account(folder, &account_email).await
                .unwrap_or(0);

            info!("Retrieved {} of {} cached emails", emails.len(), total_count);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "emails": emails,
                "folder": folder,
                "count": total_count,
            })))
        }
        Err(e) => {
            error!("Failed to get cached emails: {}", e);
            Err(ApiError::InternalError(format!("Failed to get cached emails: {}", e)))
        }
    }
}

/// Send an email via SMTP
#[derive(serde::Deserialize)]
pub struct SendEmailQueryParams {
    account_email: Option<String>,
}

pub async fn send_email(
    state: Data<DashboardState>,
    query: web::Query<SendEmailQueryParams>,
    body: web::Json<crate::dashboard::services::SendEmailRequest>,
) -> Result<impl Responder, ApiError> {
    use lettre::{Message, message::{header::ContentType, Mailbox, MultiPart, SinglePart, header}};
    use chrono::Utc;

    // REQUIRE account_email parameter - do NOT fall back to default account
    // This prevents accidentally sending from the wrong account
    let account_email = query.account_email.as_ref()
        .ok_or_else(|| ApiError::BadRequest("account_email query parameter is required".to_string()))?
        .clone();

    info!("Queueing email from account: {}", account_email);

    let request = body.into_inner();

    // Get account details to build proper From header
    let account_service = state.account_service.lock().await;
    let account = account_service.get_account(&account_email).await
        .map_err(|e| ApiError::InternalError(format!("Account not found: {}", e)))?;
    drop(account_service);

    // Build from address with properly quoted display name
    let from_mailbox: Mailbox = if account.display_name.is_empty() {
        account.email_address.parse()
            .map_err(|e| ApiError::InternalError(format!("Invalid from address: {}", e)))?
    } else {
        let quoted_name = if account.display_name.contains(|c: char| "()<>[]:;@\\,\"".contains(c)) {
            format!("\"{}\"", account.display_name.replace('\"', "\\\""))
        } else {
            account.display_name.clone()
        };
        format!("{} <{}>", quoted_name, account.email_address).parse()
            .map_err(|e| ApiError::InternalError(format!("Invalid from address: {}", e)))?
    };

    // Build email message
    let mut email_builder = Message::builder().from(from_mailbox).subject(&request.subject);

    // Add recipients
    for to_addr in &request.to {
        email_builder = email_builder.to(to_addr.parse()
            .map_err(|e| ApiError::BadRequest(format!("Invalid to address {}: {}", to_addr, e)))?);
    }
    if let Some(cc_addrs) = &request.cc {
        for cc_addr in cc_addrs {
            email_builder = email_builder.cc(cc_addr.parse()
                .map_err(|e| ApiError::BadRequest(format!("Invalid cc address {}: {}", cc_addr, e)))?);
        }
    }
    if let Some(bcc_addrs) = &request.bcc {
        for bcc_addr in bcc_addrs {
            email_builder = email_builder.bcc(bcc_addr.parse()
                .map_err(|e| ApiError::BadRequest(format!("Invalid bcc address {}: {}", bcc_addr, e)))?);
        }
    }

    // Build multipart body
    let email = if let Some(html_body) = &request.body_html {
        email_builder.multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::builder().header(header::ContentType::TEXT_PLAIN).body(request.body.clone()))
                .singlepart(SinglePart::builder().header(header::ContentType::TEXT_HTML).body(html_body.clone()))
        ).map_err(|e| ApiError::InternalError(format!("Failed to build email: {}", e)))?
    } else {
        email_builder.header(ContentType::TEXT_PLAIN).body(request.body.clone())
            .map_err(|e| ApiError::InternalError(format!("Failed to build email: {}", e)))?
    };

    // Get message ID and raw bytes
    let message_id = email.headers().get_raw("Message-ID").map(|v| v.to_string());
    let raw_email_bytes = email.formatted();

    // Create outbox queue item
    let queue_item = crate::dashboard::services::OutboxQueueItem {
        id: None,
        account_email: account_email.clone(),
        message_id: message_id.clone(),
        to_addresses: request.to.clone(),
        cc_addresses: request.cc.clone(),
        bcc_addresses: request.bcc.clone(),
        subject: request.subject.clone(),
        body_text: request.body.clone(),
        body_html: request.body_html.clone(),
        raw_email_bytes,
        status: crate::dashboard::services::OutboxStatus::Pending,
        smtp_sent: false,
        outbox_saved: false,
        sent_folder_saved: false,
        retry_count: 0,
        max_retries: 3,
        last_error: None,
        created_at: Utc::now(),
        smtp_sent_at: None,
        last_retry_at: None,
        completed_at: None,
    };

    // Enqueue the email
    match state.outbox_queue_service.enqueue(queue_item).await {
        Ok(queue_id) => {
            info!("Email queued successfully with ID: {} (will be sent asynchronously)", queue_id);

            let response = crate::dashboard::services::SendEmailResponse {
                success: true,
                message_id,
                message: format!("Email queued successfully (queue ID: {}). Background worker will send it shortly.", queue_id),
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Failed to queue email: {}", e);
            Err(ApiError::InternalError(format!("Failed to queue email: {}", e)))
        }
    }
}

/// Delete email(s) from a folder
#[derive(serde::Deserialize)]
pub struct DeleteEmailRequest {
    pub folder: String,
    pub uids: Vec<u32>,
    pub account_email: String,
}

pub async fn delete_email(
    state: Data<DashboardState>,
    body: web::Json<DeleteEmailRequest>,
) -> Result<impl Responder, ApiError> {
    let request = body.into_inner();

    info!("Deleting {} email(s) from folder {} for account {}",
          request.uids.len(), request.folder, request.account_email);

    if request.uids.is_empty() {
        return Err(ApiError::BadRequest("No UIDs provided".to_string()));
    }

    // Get account details
    let account_service = state.account_service.lock().await;
    let account = account_service.get_account(&request.account_email).await
        .map_err(|e| ApiError::InternalError(format!("Account not found: {}", e)))?;
    drop(account_service);

    // Create IMAP session for this account
    let mut session = state.imap_session_factory.create_session_for_account(&account).await
        .map_err(|e| ApiError::InternalError(format!("Failed to create IMAP session: {}", e)))?;

    // Select the folder
    session.select_folder(&request.folder).await
        .map_err(|e| ApiError::InternalError(format!("Failed to select folder {}: {}", request.folder, e)))?;

    // Delete the messages
    session.delete_messages(&request.uids).await
        .map_err(|e| ApiError::InternalError(format!("Failed to delete messages: {}", e)))?;

    info!("Successfully deleted {} email(s) from {}", request.uids.len(), request.folder);

    // Remove deleted emails from cache
    if let Err(e) = state.cache_service.delete_emails_by_uids(&request.folder, &request.uids, &request.account_email).await {
        warn!("Failed to remove deleted emails from cache: {}", e);
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "deleted_count": request.uids.len(),
        "folder": request.folder
    })))
}
