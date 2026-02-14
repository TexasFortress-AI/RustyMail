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
        }),
        serde_json::json!({
            "name": "get_attachment_content",
            "description": "Get a single attachment's content as base64 (downloads from IMAP if needed)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Message-ID of the email (provide this OR folder+uid)"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder name (required if message_id not provided)"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "Email UID (required if message_id not provided)"
                    },
                    "filename": {
                        "type": "string",
                        "description": "REQUIRED. Filename of the attachment to retrieve"
                    }
                },
                "required": ["account_id", "filename"]
            }
        }),
        serde_json::json!({
            "name": "sync_emails",
            "description": "Trigger email sync for a specific folder or all folders. Syncs emails from IMAP server into the local cache.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional. Specific folder to sync (e.g., 'INBOX', 'INBOX/resumes', 'Sent Items'). If omitted, syncs all folders."
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "get_email_synopsis",
            "description": "Get a concise synopsis of an email (subject + first sentences)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional. Folder name (default: INBOX)"
                    },
                    "uid": {
                        "type": "integer",
                        "description": "REQUIRED. Email UID"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Optional. Max sentences to extract (default: 3)"
                    }
                },
                "required": ["account_id", "uid"]
            }
        }),
        serde_json::json!({
            "name": "get_email_thread",
            "description": "Get all emails in a conversation thread by message_id (uses In-Reply-To and References headers)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    },
                    "message_id": {
                        "type": "string",
                        "description": "REQUIRED. Message-ID of any email in the thread"
                    }
                },
                "required": ["account_id", "message_id"]
            }
        }),
        serde_json::json!({
            "name": "search_by_domain",
            "description": "Search cached emails by sender/recipient domain (e.g., 'gmail.com', 'company.org')",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    },
                    "domain": {
                        "type": "string",
                        "description": "REQUIRED. Domain to search for (e.g., 'gmail.com')"
                    },
                    "search_in": {
                        "type": "array",
                        "description": "Optional. Fields to search: 'from', 'to', 'cc' (default: ['from'])",
                        "items": { "type": "string" }
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional. Max results (default: 50)"
                    }
                },
                "required": ["account_id", "domain"]
            }
        }),
        serde_json::json!({
            "name": "get_address_report",
            "description": "Get aggregated report of unique email addresses and domains for an account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "list_emails_by_flag",
            "description": "Filter cached emails by IMAP flags (Seen, Flagged, Answered, etc.)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional. Folder name (default: INBOX)"
                    },
                    "flags_include": {
                        "type": "array",
                        "description": "Optional. Emails must have ALL these flags (e.g., ['Flagged'])",
                        "items": { "type": "string" }
                    },
                    "flags_exclude": {
                        "type": "array",
                        "description": "Optional. Emails must NOT have ANY of these flags (e.g., ['Seen'] for unread)",
                        "items": { "type": "string" }
                    },
                    "unread_only": {
                        "type": "boolean",
                        "description": "Optional. Shorthand for flags_exclude=['Seen']"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional. Max results (default: 50)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional. Pagination offset (default: 0)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        serde_json::json!({
            "name": "search_by_attachment_type",
            "description": "Search for attachments matching MIME type patterns (e.g., 'image/*', 'application/pdf')",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    },
                    "mime_types": {
                        "type": "array",
                        "description": "REQUIRED. Array of MIME type patterns (e.g., ['image/*', 'application/pdf'])",
                        "items": { "type": "string" }
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional. Maximum results to return (default: 50)"
                    }
                },
                "required": ["account_id", "mime_types"]
            }
        })
    ]
}

// Query parameters for MCP tools endpoint
#[derive(Debug, Deserialize)]
pub struct McpToolsQuery {
    #[serde(default = "default_variant")]
    pub variant: String,
}

fn default_variant() -> String {
    "low-level".to_string()
}

pub async fn list_mcp_tools(
    _state: web::Data<DashboardState>,
    query: web::Query<McpToolsQuery>,
) -> Result<impl Responder, ApiError> {
    debug!("Listing MCP tools with variant: {}", query.variant);

    // Check which variant to return
    let tools = if query.variant == "high-level" {
        // Return high-level tools
        use crate::dashboard::api::high_level_tools;
        let high_level_tools = high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

        // Convert from JSON-RPC format to dashboard format
        high_level_tools.iter().map(|tool| {
            let name = tool["name"].as_str().unwrap_or("unknown");
            let description = tool["description"].as_str().unwrap_or("");
            let input_schema = &tool["inputSchema"];

            // Extract parameters from inputSchema
            let mut parameters = serde_json::json!({});
            if let Some(props) = input_schema.get("properties") {
                if let Some(props_obj) = props.as_object() {
                    for (key, value) in props_obj {
                        let desc = value.get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        parameters[key] = serde_json::Value::String(desc.to_string());
                    }
                }
            }

            serde_json::json!({
                "name": name,
                "description": description,
                "parameters": parameters
            })
        }).collect()
    } else {
        // Return low-level tools (existing implementation)
        vec![
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
            "name": "get_attachment_content",
            "description": "Get a single attachment's content as base64",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account",
                "message_id": "Message-ID (provide this OR folder+uid)",
                "folder": "Folder name (if message_id not provided)",
                "uid": "Email UID (if message_id not provided)",
                "filename": "REQUIRED. Filename of the attachment"
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
        }),
        serde_json::json!({
            "name": "sync_emails",
            "description": "Trigger email sync for a specific folder or all folders",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)",
                "folder": "Optional. Specific folder to sync (e.g., 'INBOX', 'INBOX/resumes'). If omitted, syncs all folders."
            }
        }),
        serde_json::json!({
            "name": "get_email_synopsis",
            "description": "Get a concise synopsis of an email (subject + first sentences)",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account",
                "folder": "Optional. Folder name (default: INBOX)",
                "uid": "REQUIRED. Email UID",
                "max_lines": "Optional. Max sentences to extract (default: 3)"
            }
        }),
        serde_json::json!({
            "name": "get_email_thread",
            "description": "Get all emails in a conversation thread by message_id",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account",
                "message_id": "REQUIRED. Message-ID of any email in the thread"
            }
        }),
        serde_json::json!({
            "name": "search_by_domain",
            "description": "Search cached emails by sender/recipient domain",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account",
                "domain": "REQUIRED. Domain to search for (e.g., 'gmail.com')",
                "search_in": "Optional. Array of fields: 'from', 'to', 'cc' (default: ['from'])",
                "limit": "Optional. Max results (default: 50)"
            }
        }),
        serde_json::json!({
            "name": "get_address_report",
            "description": "Get aggregated report of unique email addresses and domains",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account"
            }
        }),
        serde_json::json!({
            "name": "list_emails_by_flag",
            "description": "Filter cached emails by IMAP flags (Seen, Flagged, Answered, etc.)",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account",
                "folder": "Optional. Folder name (default: INBOX)",
                "flags_include": "Optional. Array of flags emails must have (e.g., ['Flagged'])",
                "flags_exclude": "Optional. Array of flags emails must not have (e.g., ['Seen'] for unread)",
                "unread_only": "Optional. Boolean shorthand for flags_exclude=['Seen']",
                "limit": "Optional. Max results (default: 50)",
                "offset": "Optional. Pagination offset (default: 0)"
            }
        }),
        serde_json::json!({
            "name": "search_by_attachment_type",
            "description": "Search for attachments matching MIME type patterns (e.g., 'image/*', 'application/pdf')",
            "parameters": {
                "account_id": "REQUIRED. Email address of the account (e.g., user@example.com)",
                "mime_types": "REQUIRED. Array of MIME type patterns (e.g., ['image/*', 'application/pdf'])",
                "limit": "Optional. Maximum results to return (default: 50)"
            }
        })
    ]
    }; // End of if-else for variant

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
                        } else if let Some(arr) = val.as_array() {
                            let emails = arr
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
            if let (true, Some(folder), Some(uid)) = (attachments.is_empty(), folder_opt, uid_opt) {
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
            if let (true, Some(folder), Some(uid)) = (attachments.is_empty(), folder_opt.as_ref(), uid_opt) {
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
        "get_attachment_content" => {
            use crate::dashboard::services::attachment_storage;
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let filename = match params.get("filename").and_then(|v| v.as_str()) {
                Some(f) => f.to_string(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "filename parameter is required",
                    "tool": tool_name
                })
            };

            let db_pool = match state.cache_service.db_pool.as_ref() {
                Some(pool) => pool,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Database not available",
                    "tool": tool_name
                })
            };

            // Resolve message_id from params or folder+uid
            let message_id = if let Some(mid) = params.get("message_id").and_then(|v| v.as_str()) {
                mid.to_string()
            } else {
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
                match state.cache_service.get_email_by_uid_for_account(folder, uid, &account_id).await {
                    Ok(Some(email)) => email.message_id.unwrap_or_default(),
                    Ok(None) => return serde_json::json!({
                        "success": false,
                        "error": format!("Email UID {} not found in {}", uid, folder),
                        "tool": tool_name
                    }),
                    Err(e) => return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to look up email: {}", e),
                        "tool": tool_name
                    })
                }
            };

            // Try reading from disk first; if metadata-only, fetch from IMAP
            match attachment_storage::read_attachment_content(db_pool, &account_id, &message_id, &filename).await {
                Ok((_name, content_type, content)) => {
                    serde_json::json!({
                        "success": true,
                        "filename": filename,
                        "content_type": content_type,
                        "size_bytes": content.len(),
                        "content_base64": BASE64.encode(&content),
                        "message_id": message_id,
                        "tool": tool_name
                    })
                }
                Err(attachment_storage::AttachmentError::NotFound(msg)) if msg.contains("not yet downloaded") => {
                    // Metadata exists but file not on disk - need IMAP fetch
                    let folder = params.get("folder").and_then(|v| v.as_str()).unwrap_or("INBOX");
                    let uid = params.get("uid").and_then(|v| v.as_u64()).map(|u| u as u32);

                    if let Some(uid) = uid {
                        match email_service.fetch_email_with_attachments(folder, uid, &account_id).await {
                            Ok(_) => {
                                // Retry reading after IMAP download
                                match attachment_storage::read_attachment_content(db_pool, &account_id, &message_id, &filename).await {
                                    Ok((_name, content_type, content)) => {
                                        serde_json::json!({
                                            "success": true,
                                            "filename": filename,
                                            "content_type": content_type,
                                            "size_bytes": content.len(),
                                            "content_base64": BASE64.encode(&content),
                                            "message_id": message_id,
                                            "tool": tool_name
                                        })
                                    }
                                    Err(e) => serde_json::json!({
                                        "success": false,
                                        "error": format!("Failed to read after IMAP fetch: {}", e),
                                        "tool": tool_name
                                    })
                                }
                            }
                            Err(e) => serde_json::json!({
                                "success": false,
                                "error": format!("Failed to fetch from IMAP: {}", e),
                                "tool": tool_name
                            })
                        }
                    } else {
                        serde_json::json!({
                            "success": false,
                            "error": "Attachment not yet downloaded. Provide folder+uid to trigger IMAP fetch.",
                            "tool": tool_name
                        })
                    }
                }
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("Failed to read attachment: {}", e),
                    "tool": tool_name
                })
            }
        }
        // === Job Management Tools ===
        "list_jobs" => {
            use crate::dashboard::services::jobs::JobStatus;
            let status_filter = params.get("status_filter").and_then(|v| v.as_str());

            let jobs: Vec<_> = state.jobs.iter()
                .filter(|entry| {
                    match status_filter {
                        Some("running") => matches!(entry.value().status, JobStatus::Running),
                        Some("completed") => matches!(entry.value().status, JobStatus::Completed(_)),
                        Some("failed") => matches!(entry.value().status, JobStatus::Failed(_)),
                        _ => true, // No filter, return all
                    }
                })
                .map(|entry| {
                    let job = entry.value();
                    serde_json::json!({
                        "job_id": job.job_id,
                        "instruction": job.instruction,
                        "status": &job.status,
                        "elapsed_seconds": job.started_at.elapsed().as_secs()
                    })
                })
                .collect();

            serde_json::json!({
                "success": true,
                "data": {
                    "jobs": jobs,
                    "total": jobs.len()
                },
                "tool": tool_name
            })
        }
        "get_job_status" => {
            let job_id = match params.get("job_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing required parameter: job_id",
                    "tool": tool_name
                }),
            };

            match state.jobs.get(job_id) {
                Some(job) => serde_json::json!({
                    "success": true,
                    "data": {
                        "job_id": job.job_id,
                        "instruction": job.instruction,
                        "status": &job.status,
                        "elapsed_seconds": job.started_at.elapsed().as_secs()
                    },
                    "tool": tool_name
                }),
                None => serde_json::json!({
                    "success": false,
                    "error": format!("Job not found: {}", job_id),
                    "tool": tool_name
                }),
            }
        }
        "cancel_job" => {
            use crate::dashboard::services::jobs::JobStatus;
            let job_id = match params.get("job_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return serde_json::json!({
                    "success": false,
                    "error": "Missing required parameter: job_id",
                    "tool": tool_name
                }),
            };

            // Get current status before removal
            let job_info = state.jobs.get(job_id).map(|job| {
                let was_running = matches!(job.status, JobStatus::Running);
                serde_json::json!({
                    "job_id": job.job_id,
                    "status": &job.status,
                    "was_running": was_running,
                    "elapsed_seconds": job.started_at.elapsed().as_secs()
                })
            });

            match job_info {
                Some(info) => {
                    // Remove the job from the map
                    state.jobs.remove(job_id);
                    serde_json::json!({
                        "success": true,
                        "message": "Job cancelled",
                        "data": info,
                        "tool": tool_name
                    })
                }
                None => serde_json::json!({
                    "success": false,
                    "error": format!("Job not found: {}", job_id),
                    "tool": tool_name
                }),
            }
        }
        "get_email_synopsis" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let folder = params.get("folder").and_then(|v| v.as_str()).unwrap_or("INBOX");
            let uid = match params.get("uid").and_then(|v| v.as_u64()).map(|u| u as u32) {
                Some(u) => u,
                None => return serde_json::json!({
                    "success": false,
                    "error": "uid parameter is required",
                    "tool": tool_name
                })
            };
            let max_lines = params.get("max_lines").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

            match state.cache_service.get_email_by_uid_for_account(folder, uid, &account_id).await {
                Ok(Some(email)) => {
                    let subject = email.subject.as_deref().unwrap_or("(no subject)");
                    let synopsis = match &email.body_text {
                        Some(body) => {
                            let sentences: Vec<&str> = body
                                .split(|c: char| c == '.' || c == '!' || c == '?')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty() && s.len() > 5)
                                .take(max_lines)
                                .collect();
                            if sentences.is_empty() {
                                body.lines()
                                    .map(|l| l.trim())
                                    .filter(|l| !l.is_empty())
                                    .take(max_lines)
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else {
                                sentences.join(". ") + "."
                            }
                        }
                        None => "(no body text available)".to_string(),
                    };
                    serde_json::json!({
                        "success": true,
                        "uid": uid,
                        "folder": folder,
                        "subject": subject,
                        "synopsis": synopsis,
                        "from": email.from_address,
                        "date": email.date,
                        "has_attachments": email.has_attachments,
                        "tool": tool_name
                    })
                }
                Ok(None) => serde_json::json!({
                    "success": false,
                    "error": format!("Email UID {} not found in {}", uid, folder),
                    "tool": tool_name
                }),
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("Failed to fetch email: {}", e),
                    "tool": tool_name
                })
            }
        }
        "get_email_thread" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let message_id = match params.get("message_id").and_then(|v| v.as_str()) {
                Some(mid) => mid.to_string(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "message_id parameter is required",
                    "tool": tool_name
                })
            };

            match state.cache_service.get_thread_emails(&message_id, &account_id).await {
                Ok(emails) => {
                    let thread: Vec<serde_json::Value> = emails.iter().map(|e| {
                        serde_json::json!({
                            "uid": e.uid,
                            "message_id": e.message_id,
                            "subject": e.subject,
                            "from_address": e.from_address,
                            "from_name": e.from_name,
                            "date": e.date,
                            "in_reply_to": e.in_reply_to,
                            "has_attachments": e.has_attachments,
                            "flags": e.flags,
                        })
                    }).collect();
                    serde_json::json!({
                        "success": true,
                        "thread_count": thread.len(),
                        "thread": thread,
                        "tool": tool_name
                    })
                }
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("Failed to fetch thread: {}", e),
                    "tool": tool_name
                })
            }
        }
        "search_by_domain" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let domain = match params.get("domain").and_then(|v| v.as_str()) {
                Some(d) => d.to_string(),
                None => return serde_json::json!({
                    "success": false,
                    "error": "domain parameter is required",
                    "tool": tool_name
                })
            };

            let search_in: Vec<&str> = params.get("search_in")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|s| s.as_str()).collect())
                .unwrap_or_else(|| vec!["from"]);
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

            match state.cache_service.search_by_domain(&domain, &search_in, &account_id, limit).await {
                Ok(emails) => {
                    let results: Vec<serde_json::Value> = emails.iter().map(|e| {
                        serde_json::json!({
                            "uid": e.uid,
                            "subject": e.subject,
                            "from_address": e.from_address,
                            "from_name": e.from_name,
                            "date": e.date,
                            "flags": e.flags,
                            "has_attachments": e.has_attachments,
                        })
                    }).collect();
                    serde_json::json!({
                        "success": true,
                        "domain": domain,
                        "count": results.len(),
                        "emails": results,
                        "tool": tool_name
                    })
                }
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("Domain search failed: {}", e),
                    "tool": tool_name
                })
            }
        }
        "get_address_report" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            match state.cache_service.get_address_report(&account_id).await {
                Ok(report) => {
                    serde_json::json!({
                        "success": true,
                        "report": report,
                        "tool": tool_name
                    })
                }
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("Failed to generate report: {}", e),
                    "tool": tool_name
                })
            }
        }
        "list_emails_by_flag" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let folder = params.get("folder").and_then(|v| v.as_str()).unwrap_or("INBOX");
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let flags_include: Vec<String> = params.get("flags_include")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let flags_exclude: Vec<String> = params.get("flags_exclude")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            // Shorthand: unread_only adds "Seen" to exclude list
            if params.get("unread_only").and_then(|v| v.as_bool()).unwrap_or(false) {
                let mut exclude = flags_exclude;
                if !exclude.contains(&"Seen".to_string()) {
                    exclude.push("Seen".to_string());
                }
                match state.cache_service.get_cached_emails_by_flags(
                    folder, &account_id, &flags_include, &exclude, limit, offset,
                ).await {
                    Ok(emails) => serde_json::json!({
                        "success": true,
                        "data": emails,
                        "count": emails.len(),
                        "folder": folder,
                        "tool": tool_name
                    }),
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": format!("Failed to filter emails: {}", e),
                        "tool": tool_name
                    })
                }
            } else {
                match state.cache_service.get_cached_emails_by_flags(
                    folder, &account_id, &flags_include, &flags_exclude, limit, offset,
                ).await {
                    Ok(emails) => serde_json::json!({
                        "success": true,
                        "data": emails,
                        "count": emails.len(),
                        "folder": folder,
                        "tool": tool_name
                    }),
                    Err(e) => serde_json::json!({
                        "success": false,
                        "error": format!("Failed to filter emails: {}", e),
                        "tool": tool_name
                    })
                }
            }
        }
        "search_by_attachment_type" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let mime_types: Vec<String> = params.get("mime_types")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

            let pool = state.cache_service.db_pool.as_ref();
            match pool {
                Some(pool) => {
                    match crate::dashboard::services::attachment_storage::search_by_attachment_type(
                        pool, &account_id, &mime_types, limit
                    ).await {
                        Ok(results) => serde_json::json!({
                            "success": true,
                            "data": results,
                            "count": results.len(),
                            "tool": tool_name
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Search failed: {}", e),
                            "tool": tool_name
                        })
                    }
                }
                None => serde_json::json!({
                    "success": false,
                    "error": "Database not available",
                    "tool": tool_name
                })
            }
        }
        "sync_emails" => {
            let account_id = match get_account_id_to_use(&params, &state_data).await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to determine account: {}", e),
                    "tool": tool_name
                })
            };

            let folder = params.get("folder").and_then(|v| v.as_str()).map(|s| s.to_string());
            let sync_service = state.sync_service.clone();

            match folder {
                Some(ref f) => {
                    info!("MCP sync_emails: syncing folder '{}' for account '{}'", f, account_id);
                    match sync_service.sync_folder(&account_id, f).await {
                        Ok(()) => serde_json::json!({
                            "success": true,
                            "message": format!("Synced folder '{}' for account '{}'", f, account_id),
                            "tool": tool_name
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Failed to sync folder '{}': {}", f, e),
                            "tool": tool_name
                        })
                    }
                }
                None => {
                    info!("MCP sync_emails: syncing all folders for account '{}'", account_id);
                    match sync_service.sync_all_folders(&account_id).await {
                        Ok(()) => serde_json::json!({
                            "success": true,
                            "message": format!("Synced all folders for account '{}'", account_id),
                            "tool": tool_name
                        }),
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Failed to sync all folders: {}", e),
                            "tool": tool_name
                        })
                    }
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

// Query parameter for MCP tool variant
#[derive(Debug, Deserialize)]
pub struct McpExecuteQuery {
    #[serde(default)]
    pub variant: String,
}

// HTTP Handler for executing MCP tools - wraps execute_mcp_tool_inner
pub async fn execute_mcp_tool(
    state: web::Data<DashboardState>,
    query: web::Query<McpExecuteQuery>,
    req: web::Json<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    let tool_name = req.get("tool")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Missing tool name".to_string()))?;

    let params = req.get("parameters")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Check if this is a high-level tool request
    let result = if query.variant == "high-level" {
        // Route to high-level tools handler
        use crate::dashboard::api::high_level_tools;
        high_level_tools::execute_high_level_tool(state.get_ref(), tool_name, params).await
    } else {
        // Call the standard MCP tool handler
        execute_mcp_tool_inner(state.get_ref(), tool_name, params).await
    };

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
    #[serde(default)]
    pub model_name: Option<String>,
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
    debug!("Handling POST /api/dashboard/ai/providers/set with provider: {}, model: {:?}",
           req.provider_name, req.model_name);

    // Get the model name - use provided one or get current model for this provider
    let model_name = match &req.model_name {
        Some(name) => name.clone(),
        None => {
            // Look up the current model from provider configs
            let providers = state.ai_service.list_providers().await;
            providers.iter()
                .find(|p| p.name == req.provider_name)
                .map(|p| p.model.clone())
                .unwrap_or_else(|| "default".to_string())
        }
    };

    // Try to persist to database if pool is available
    if let Some(pool) = state.cache_service.db_pool.as_ref() {
        state.ai_service
            .set_current_provider_with_persistence(pool, req.provider_name.clone(), model_name.clone())
            .await
            .map_err(|e| ApiError::BadRequest(e))?;

        info!("Persisted chatbot provider selection to database: provider={}, model={}",
              req.provider_name, model_name);
    } else {
        // Fallback to in-memory only if no database
        warn!("No database pool available, provider selection will not persist across restarts");
        state.ai_service
            .set_current_provider(req.provider_name.clone())
            .await
            .map_err(|e| ApiError::BadRequest(e))?;
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Successfully switched to provider: {} with model: {}", req.provider_name, model_name),
        "current_provider": req.provider_name,
        "current_model": model_name
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

// Request types for model configuration endpoints
#[derive(Debug, Deserialize)]
pub struct SetModelConfigRequest {
    pub role: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

// Handler for getting all model configurations (tool-calling, drafting)
pub async fn get_model_configs(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/model-configs");

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::model_config;

    match model_config::get_all_model_configs(pool).await {
        Ok(configs) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "configs": configs
        }))),
        Err(e) => {
            error!("Failed to get model configurations: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to get model configurations: {:?}", e)))
        }
    }
}

// Handler for setting a model configuration
pub async fn set_model_config(
    req: web::Json<SetModelConfigRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/model-configs with role: {}", req.role);

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::model_config::{ModelConfiguration, set_model_config as save_model_config};

    let mut config = ModelConfiguration::new(&req.role, &req.provider, &req.model_name);

    if let Some(ref base_url) = req.base_url {
        config = config.with_base_url(base_url);
    }

    if let Some(ref api_key) = req.api_key {
        config = config.with_api_key(api_key);
    }

    match save_model_config(pool, &config).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Successfully set {} model configuration", req.role),
            "config": config
        }))),
        Err(e) => {
            error!("Failed to set model configuration: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to set model configuration: {:?}", e)))
        }
    }
}

// Handler for getting models for a specific provider
pub async fn get_models_for_provider(
    query: web::Query<GetModelsForProviderQuery>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/models-for-provider?provider={}", query.provider);

    // Get available models from the specified provider
    let available_models = match state.ai_service.get_available_models_for_provider(&query.provider).await {
        Ok(models) => models,
        Err(e) => {
            warn!("Failed to fetch models from provider {}: {:?}", query.provider, e);
            // Return empty list if API call fails
            vec![]
        }
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "provider": query.provider,
        "available_models": available_models
    })))
}

#[derive(Debug, Deserialize)]
pub struct GetModelsForProviderQuery {
    pub provider: String,
}

// ============================================================================
// Sampler Configuration API
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GetSamplerConfigQuery {
    pub provider: String,
    pub model_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SamplerConfigRequest {
    pub provider: String,
    pub model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typical_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub think_mode: bool,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub provider_options: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Get sampler configuration for a specific provider/model
pub async fn get_sampler_config(
    query: web::Query<GetSamplerConfigQuery>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/sampler-configs?provider={}&model_name={}",
           query.provider, query.model_name);

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::sampler_config;

    match sampler_config::get_sampler_config(pool, &query.provider, &query.model_name).await {
        Ok(config) => Ok(HttpResponse::Ok().json(config)),
        Err(e) => {
            error!("Failed to get sampler config: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to get sampler config: {:?}", e)))
        }
    }
}

/// List all sampler configurations
pub async fn list_sampler_configs(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/sampler-configs/list");

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::sampler_config;

    match sampler_config::list_sampler_configs(pool).await {
        Ok(configs) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "configs": configs
        }))),
        Err(e) => {
            error!("Failed to list sampler configs: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to list sampler configs: {:?}", e)))
        }
    }
}

/// Save sampler configuration
pub async fn set_sampler_config(
    req: web::Json<SamplerConfigRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/sampler-configs for {}/{}",
           req.provider, req.model_name);

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::sampler_config::{SamplerConfig, save_sampler_config};

    let mut config = SamplerConfig::new(&req.provider, &req.model_name);
    config.temperature = req.temperature;
    config.top_p = req.top_p;
    config.top_k = req.top_k;
    config.min_p = req.min_p;
    config.typical_p = req.typical_p;
    config.repeat_penalty = req.repeat_penalty;
    config.num_ctx = req.num_ctx;
    config.max_tokens = req.max_tokens;
    config.think_mode = req.think_mode;
    config.stop_sequences = req.stop_sequences.clone();
    config.system_prompt = req.system_prompt.clone();
    config.provider_options = req.provider_options.clone();
    config.description = req.description.clone();

    match save_sampler_config(pool, &config).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Successfully saved sampler config for {}/{}", req.provider, req.model_name),
            "config": config
        }))),
        Err(e) => {
            error!("Failed to save sampler config: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to save sampler config: {:?}", e)))
        }
    }
}

/// Delete sampler configuration
pub async fn delete_sampler_config(
    query: web::Query<GetSamplerConfigQuery>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling DELETE /api/dashboard/ai/sampler-configs?provider={}&model_name={}",
           query.provider, query.model_name);

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::sampler_config;

    match sampler_config::delete_sampler_config(pool, &query.provider, &query.model_name).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Successfully deleted sampler config for {}/{}", query.provider, query.model_name)
        }))),
        Err(e) => {
            error!("Failed to delete sampler config: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to delete sampler config: {:?}", e)))
        }
    }
}

/// Get environment default sampler values
pub async fn get_env_defaults() -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/sampler-configs/defaults");

    use crate::dashboard::services::ai::sampler_config;

    let defaults = sampler_config::get_env_defaults();
    Ok(HttpResponse::Ok().json(defaults))
}

/// Get recommended sampler configuration presets
pub async fn get_sampler_presets() -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/sampler-configs/presets");

    use crate::dashboard::services::ai::sampler_config;

    let presets = sampler_config::get_recommended_presets();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "categories": presets
    })))
}

#[derive(Debug, Deserialize)]
pub struct ImportPresetsRequest {
    pub presets: Vec<ImportPresetItem>,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Deserialize)]
pub struct ImportPresetItem {
    pub provider: String,
    pub model_name: String,
}

/// Import selected presets into the database
pub async fn import_sampler_presets(
    req: web::Json<ImportPresetsRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/sampler-configs/presets/import with {} presets",
           req.presets.len());

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return Err(ApiError::InternalError("Database not initialized".to_string())),
    };

    use crate::dashboard::services::ai::sampler_config;

    // Get all recommended presets
    let all_presets = sampler_config::get_recommended_presets();

    // Filter to only the selected presets
    let selected_presets: Vec<_> = all_presets.iter()
        .flat_map(|cat| cat.presets.iter())
        .filter(|preset| {
            req.presets.iter().any(|item| {
                item.provider == preset.provider && item.model_name == preset.model_name
            })
        })
        .cloned()
        .collect();

    if selected_presets.is_empty() {
        return Err(ApiError::BadRequest("No matching presets found".to_string()));
    }

    match sampler_config::import_presets(pool, &selected_presets, req.overwrite).await {
        Ok(result) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Imported {} presets, skipped {}", result.imported, result.skipped),
            "imported": result.imported,
            "skipped": result.skipped
        }))),
        Err(e) => {
            error!("Failed to import presets: {:?}", e);
            Err(ApiError::InternalError(format!("Failed to import presets: {:?}", e)))
        }
    }
}

/// Trigger an email sync using the separate sync process.
/// This spawns the rustymail-sync binary which exits after sync, ensuring memory is returned to OS.
///
/// Query parameters:
///   - account_id: Optional email address to sync only one account
///   - folder: Optional folder name to sync only one folder (requires account_id)
///
/// If a sync is already running (exit code 2), returns success with status "in_progress".
pub async fn trigger_email_sync(
    state: Data<DashboardState>,
    query: web::Query<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    // Extract optional account and folder from query params
    let account_id = match get_account_id_to_use(&query.0, &state).await {
        Ok(id) => Some(id),
        Err(_) => None, // No account specified, will sync all
    };
    let folder = query.get("folder").and_then(|v| v.as_str()).map(|s| s.to_string());

    let mode_desc = match (&account_id, &folder) {
        (Some(acc), Some(f)) => format!("account {} folder {}", acc, f),
        (Some(acc), None) => format!("account {}", acc),
        (None, _) => "all accounts".to_string(),
    };
    info!("Triggering email sync via separate process for {}", mode_desc);

    // Find the sync binary - check multiple locations
    let sync_binary = if std::path::Path::new("./target/release/rustymail-sync").exists() {
        "./target/release/rustymail-sync"
    } else if std::path::Path::new("./target/debug/rustymail-sync").exists() {
        "./target/debug/rustymail-sync"
    } else if std::path::Path::new("./rustymail-sync").exists() {
        "./rustymail-sync"
    } else {
        "rustymail-sync"
    };

    // Build command with optional arguments
    let mut cmd = std::process::Command::new(sync_binary);
    if let Some(ref acc) = account_id {
        cmd.arg("--account").arg(acc);
    }
    if let Some(ref f) = folder {
        cmd.arg("--folder").arg(f);
    }

    // Spawn the sync process and wait for it to complete (or detect already running)
    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            info!("Spawned sync process (pid: {})", pid);

            // Wait briefly to see if it exits immediately with "already running" code
            std::thread::sleep(std::time::Duration::from_millis(100));

            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited already
                    if status.code() == Some(2) {
                        // Exit code 2 = already running
                        info!("Sync process reports another sync is already in progress");
                        Ok(HttpResponse::Ok().json(serde_json::json!({
                            "message": "Email sync is already in progress",
                            "status": "in_progress"
                        })))
                    } else if status.success() {
                        // Completed very quickly (unlikely but possible for empty sync)
                        Ok(HttpResponse::Ok().json(serde_json::json!({
                            "message": format!("Email sync completed for {}", mode_desc),
                            "status": "completed"
                        })))
                    } else {
                        // Some other error
                        error!("Sync process exited with error code: {:?}", status.code());
                        Err(ApiError::InternalError(format!("Sync process failed with exit code: {:?}", status.code())))
                    }
                }
                Ok(None) => {
                    // Still running - this is the normal case
                    Ok(HttpResponse::Ok().json(serde_json::json!({
                        "message": format!("Email sync started for {}", mode_desc),
                        "status": "syncing",
                        "pid": pid
                    })))
                }
                Err(e) => {
                    error!("Failed to check sync process status: {}", e);
                    // Assume it's running
                    Ok(HttpResponse::Ok().json(serde_json::json!({
                        "message": format!("Email sync started for {}", mode_desc),
                        "status": "syncing",
                        "pid": pid
                    })))
                }
            }
        }
        Err(e) => {
            error!("Failed to spawn sync process '{}': {}", sync_binary, e);
            Err(ApiError::InternalError(format!("Failed to start sync process: {}", e)))
        }
    }
}

/// Resync only FLAGS from the IMAP server for cached emails (lightweight, no body download).
pub async fn sync_flags(
    state: Data<DashboardState>,
    query: web::Query<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    let account_id = get_account_id_to_use(&query.0, &state).await?;
    let folder = query.get("folder").and_then(|v| v.as_str()).unwrap_or("INBOX");

    info!("Triggering flag resync for account {} folder {}", account_id, folder);

    let sync_service = state.sync_service.clone();
    let account_id_owned = account_id.clone();
    let folder_owned = folder.to_string();

    tokio::spawn(async move {
        if let Err(e) = sync_service.sync_flags_for_folder(&account_id_owned, &folder_owned).await {
            error!("Flag resync failed for {}/{}: {}", account_id_owned, folder_owned, e);
        }
    });

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Flag resync started for folder {} on account {}", folder, account_id),
        "status": "syncing_flags"
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

/// List folders from the local cache database (no IMAP connection needed)
pub async fn list_cached_folders(
    state: Data<DashboardState>,
    query: web::Query<EmailQueryParams>,
) -> Result<impl Responder, ApiError> {
    let account_id = match query.account_id.as_ref() {
        Some(id) => id.clone(),
        None => {
            let account_service = state.account_service.lock().await;
            match account_service.get_default_account().await {
                Ok(Some(account)) => account.email_address,
                Ok(None) => return Err(ApiError::NotFound("No default account configured".to_string())),
                Err(e) => return Err(ApiError::InternalError(format!("Failed to get default account: {}", e))),
            }
        }
    };

    info!("Listing cached folders for account: {}", account_id);

    match state.cache_service.get_all_cached_folders_for_account(&account_id).await {
        Ok(folders) => {
            let folder_names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
            info!("Found {} cached folders for account {}", folders.len(), account_id);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "account_id": account_id,
                "folders": folder_names,
                "folder_details": folders,
            })))
        }
        Err(e) => {
            error!("Failed to list cached folders for account {}: {}", account_id, e);
            Err(ApiError::InternalError(format!("Failed to list cached folders: {}", e)))
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
    let session = state.imap_session_factory.create_session_for_account(&account).await
        .map_err(|e| ApiError::InternalError(format!("Failed to create IMAP session: {}", e)))?;

    // Select the folder
    session.select_folder(&request.folder).await
        .map_err(|e| ApiError::InternalError(format!("Failed to select folder {}: {}", request.folder, e)))?;

    // Delete the messages
    session.delete_messages(&request.uids).await
        .map_err(|e| ApiError::InternalError(format!("Failed to delete messages: {}", e)))?;

    // IMPORTANT: Logout to release BytePool buffers and prevent memory leak
    if let Err(e) = session.logout().await {
        warn!("Failed to logout IMAP session: {}", e);
    }

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

// ============================================================================
// Jobs API Handlers
// ============================================================================

/// Query parameters for listing jobs
#[derive(Debug, Deserialize)]
pub struct JobsQueryParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// Get all background jobs
pub async fn get_jobs(
    query: web::Query<JobsQueryParams>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/jobs with query: {:?}", query);

    // First try to get jobs from persistence service if available
    if let Some(ref persistence) = state.job_persistence {
        let jobs = persistence.get_all_jobs(query.status.as_deref(), query.limit)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to get jobs: {}", e)))?;

        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "jobs": jobs,
            "source": "database"
        })));
    }

    // Fall back to in-memory jobs
    let jobs: Vec<_> = state.jobs.iter()
        .filter(|entry| {
            if let Some(ref status_filter) = query.status {
                match (&entry.status, status_filter.as_str()) {
                    (crate::dashboard::services::jobs::JobStatus::Running, "running") => true,
                    (crate::dashboard::services::jobs::JobStatus::Completed(_), "completed") => true,
                    (crate::dashboard::services::jobs::JobStatus::Failed(_), "failed") => true,
                    _ => false,
                }
            } else {
                true
            }
        })
        .take(query.limit.unwrap_or(100) as usize)
        .map(|entry| entry.value().clone())
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "jobs": jobs,
        "source": "memory"
    })))
}

/// Get a specific job by ID
pub async fn get_job(
    path: web::Path<String>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    let job_id = path.into_inner();
    debug!("Handling GET /api/dashboard/jobs/{}", job_id);

    // First try persistence service
    if let Some(ref persistence) = state.job_persistence {
        if let Some(job) = persistence.get_job(&job_id)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to get job: {}", e)))?
        {
            return Ok(HttpResponse::Ok().json(job));
        }
    }

    // Fall back to in-memory
    if let Some(entry) = state.jobs.get(&job_id) {
        return Ok(HttpResponse::Ok().json(entry.value().clone()));
    }

    Err(ApiError::NotFound(format!("Job {} not found", job_id)))
}

/// Cancel a running job
#[derive(Debug, Deserialize)]
pub struct CancelJobRequest {
    pub job_id: String,
}

pub async fn cancel_job(
    req: web::Json<CancelJobRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    info!("Handling POST /api/dashboard/jobs/cancel for job: {}", req.job_id);

    // Update in database if persistence is enabled
    if let Some(ref persistence) = state.job_persistence {
        let cancelled = persistence.cancel_job(&req.job_id)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to cancel job: {}", e)))?;

        if !cancelled {
            return Err(ApiError::BadRequest(format!("Job {} is not running or not found", req.job_id)));
        }
    }

    // Update in-memory state
    if let Some(mut entry) = state.jobs.get_mut(&req.job_id) {
        if matches!(entry.status, crate::dashboard::services::jobs::JobStatus::Running) {
            entry.status = crate::dashboard::services::jobs::JobStatus::Failed("Cancelled by user".to_string());
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "job_id": req.job_id,
        "message": "Job cancelled successfully"
    })))
}

/// Request body for starting a process_email_instructions job
#[derive(Debug, Deserialize)]
pub struct StartProcessEmailInstructionsRequest {
    pub instruction: String,
    pub account_id: String,
    pub folder: Option<String>,
}

/// Start a new process_email_instructions job
pub async fn start_process_email_instructions(
    req: web::Json<StartProcessEmailInstructionsRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    info!("Handling POST /api/dashboard/jobs/process-emails with instruction: {} for account: {}",
          req.instruction, req.account_id);

    let folder = req.folder.clone().unwrap_or_else(|| "INBOX".to_string());

    // Build the arguments for the high-level tool
    let arguments = serde_json::json!({
        "instruction": req.instruction,
        "account_id": req.account_id,
        "folder": folder
    });

    // Call the high-level tool which internally handles job creation and spawning
    let result = crate::dashboard::api::high_level_tools::handle_process_email_instructions(
        &state,
        arguments,
    ).await;

    // The result contains success and job_id fields
    if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        let job_id = result.get("job_id").and_then(|v| v.as_str()).unwrap_or("unknown");
        Ok(HttpResponse::Accepted().json(serde_json::json!({
            "job_id": job_id,
            "status": "running",
            "message": "Job started successfully"
        })))
    } else {
        let error = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        Err(ApiError::BadRequest(error.to_string()))
    }
}
