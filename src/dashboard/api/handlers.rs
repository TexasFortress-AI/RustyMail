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
    debug!("Handling POST /api/dashboard/chatbot/query with body: {:?}", req);

    let response = state.ai_service.process_query(req.0)
        .await
        .map_err(|e| ApiError::InternalError(format!("AI service error: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

// Handler for listing available MCP tools
pub async fn list_mcp_tools(
    _state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    // List of MCP tools available in the system
    let tools = vec![
        serde_json::json!({
            "name": "list_folders",
            "description": "List all email folders in the account",
            "parameters": {}
        }),
        serde_json::json!({
            "name": "list_folders_hierarchical",
            "description": "List folders with hierarchical structure",
            "parameters": {}
        }),
        serde_json::json!({
            "name": "search_emails",
            "description": "Search for emails matching criteria",
            "parameters": {
                "folder": "Folder to search in (e.g., INBOX)",
                "query": "Search query (e.g., FROM user@example.com)",
                "max_results": "Maximum number of results (optional)"
            }
        }),
        serde_json::json!({
            "name": "fetch_emails_with_mime",
            "description": "Fetch email content with MIME data",
            "parameters": {
                "folder": "Folder containing the email",
                "uid": "Email UID"
            }
        }),
        serde_json::json!({
            "name": "atomic_move_message",
            "description": "Move a single message to another folder",
            "parameters": {
                "source_folder": "Source folder",
                "target_folder": "Target folder",
                "uid": "Message UID to move"
            }
        }),
        serde_json::json!({
            "name": "atomic_batch_move",
            "description": "Move multiple messages to another folder",
            "parameters": {
                "source_folder": "Source folder",
                "target_folder": "Target folder",
                "uids": "Comma-separated list of UIDs"
            }
        }),
        serde_json::json!({
            "name": "mark_as_deleted",
            "description": "Mark messages as deleted",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs"
            }
        }),
        serde_json::json!({
            "name": "delete_messages",
            "description": "Permanently delete messages",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs"
            }
        }),
        serde_json::json!({
            "name": "undelete_messages",
            "description": "Unmark messages as deleted",
            "parameters": {
                "folder": "Folder containing messages",
                "uids": "Comma-separated list of UIDs"
            }
        }),
        serde_json::json!({
            "name": "expunge",
            "description": "Expunge deleted messages from folder",
            "parameters": {
                "folder": "Folder to expunge"
            }
        }),
        // Cache-based tools
        serde_json::json!({
            "name": "list_cached_emails",
            "description": "List cached emails from database",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "limit": "Maximum number of emails (default: 20)",
                "offset": "Pagination offset (default: 0)"
            }
        }),
        serde_json::json!({
            "name": "get_email_by_uid",
            "description": "Get full cached email by UID",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "uid": "Email UID"
            }
        }),
        serde_json::json!({
            "name": "get_email_by_index",
            "description": "Get cached email by position index",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "index": "Zero-based position index"
            }
        }),
        serde_json::json!({
            "name": "count_emails_in_folder",
            "description": "Count total emails in cached folder",
            "parameters": {
                "folder": "Folder name (default: INBOX)"
            }
        }),
        serde_json::json!({
            "name": "get_folder_stats",
            "description": "Get statistics about cached folder",
            "parameters": {
                "folder": "Folder name (default: INBOX)"
            }
        }),
        serde_json::json!({
            "name": "search_cached_emails",
            "description": "Search within cached emails",
            "parameters": {
                "folder": "Folder name (default: INBOX)",
                "query": "Search query text",
                "limit": "Maximum number of results (default: 20)"
            }
        })
    ];

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tools": tools
    })))
}

// Handler for executing MCP tools
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

    debug!("Executing MCP tool: {} with params: {:?}", tool_name, params);

    // Get the email service from the state
    let email_service = state.email_service.clone();

    // Execute the appropriate tool
    let result = match tool_name {
        "list_folders" => {
            match email_service.list_folders().await {
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
            // For now, just use regular list_folders since hierarchical is not implemented
            match email_service.list_folders().await {
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
        "search_emails" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let query = params.get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("ALL");

            // search_emails returns UIDs, not full emails
            match email_service.search_emails(folder, query).await {
                Ok(uids) => {
                    // Optionally limit results
                    let max_results = params.get("max_results")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize);

                    let limited_uids = if let Some(max) = max_results {
                        uids.into_iter().take(max).collect::<Vec<_>>()
                    } else {
                        uids
                    };

                    serde_json::json!({
                        "success": true,
                        "data": limited_uids,
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
        }
        "fetch_emails_with_mime" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'folder' parameter".to_string()))?;
            let uid = params.get("uid")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uid' parameter".to_string()))? as u32;

            // fetch_emails expects an array of UIDs
            match email_service.fetch_emails(folder, &[uid]).await {
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

            match state.cache_service.get_cached_emails(folder, limit, offset).await {
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
        "get_email_by_uid" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let uid = params.get("uid")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            if let Some(uid) = uid {
                match state.cache_service.get_email_by_uid(folder, uid).await {
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
        "get_email_by_index" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");
            let index = params.get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            if let Some(index) = index {
                // Get emails sorted by date DESC, then select by index
                match state.cache_service.get_cached_emails(folder, index + 1, index).await {
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
        "count_emails_in_folder" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");

            match state.cache_service.count_emails_in_folder(folder).await {
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
        "get_folder_stats" => {
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or("INBOX");

            match state.cache_service.get_folder_stats(folder).await {
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

            if let Some(query) = query {
                match state.cache_service.search_cached_emails(folder, query, limit).await {
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
        "atomic_move_message" => {
            let uid = params.get("uid")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uid' parameter".to_string()))? as u32;
            let from_folder = params.get("from_folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'from_folder' parameter".to_string()))?;
            let to_folder = params.get("to_folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'to_folder' parameter".to_string()))?;

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
            let uids = params.get("uids")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uids' parameter".to_string()))?
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect::<Vec<u32>>();
            let from_folder = params.get("from_folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'from_folder' parameter".to_string()))?;
            let to_folder = params.get("to_folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'to_folder' parameter".to_string()))?;

            if uids.is_empty() {
                return Err(ApiError::BadRequest("'uids' parameter cannot be empty".to_string()));
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
        "mark_as_deleted" => {
            let uids = params.get("uids")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uids' parameter".to_string()))?
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect::<Vec<u32>>();
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'folder' parameter".to_string()))?;

            if uids.is_empty() {
                return Err(ApiError::BadRequest("'uids' parameter cannot be empty".to_string()));
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
            let uids = params.get("uids")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uids' parameter".to_string()))?
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect::<Vec<u32>>();
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'folder' parameter".to_string()))?;

            if uids.is_empty() {
                return Err(ApiError::BadRequest("'uids' parameter cannot be empty".to_string()));
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
            let uids = params.get("uids")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ApiError::BadRequest("Missing 'uids' parameter".to_string()))?
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect::<Vec<u32>>();
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'folder' parameter".to_string()))?;

            if uids.is_empty() {
                return Err(ApiError::BadRequest("'uids' parameter cannot be empty".to_string()));
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
            let folder = params.get("folder")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::BadRequest("Missing 'folder' parameter".to_string()))?;

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
        _ => {
            // For other tools not yet implemented
            serde_json::json!({
                "success": false,
                "message": format!("Tool '{}' execution not yet implemented", tool_name),
                "tool": tool_name
            })
        }
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
) -> Result<impl Responder, ApiError> {
    info!("Triggering full email sync for all folders");

    // Clone the sync service Arc to move into the async task
    let sync_service = state.sync_service.clone();

    // Spawn the sync task in the background
    tokio::spawn(async move {
        let folders = vec!["INBOX", "INBOX.Sent", "INBOX.Drafts", "INBOX.Trash", "INBOX.spam"];
        for folder in folders {
            info!("Starting full sync of {}", folder);
            match sync_service.full_sync_folder(folder).await {
                Ok(()) => info!("{} sync completed successfully", folder),
                Err(e) => error!("{} sync failed: {}", folder, e),
            }
        }
        info!("All folder syncs completed");
    });

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email sync started in background for all folders",
        "status": "syncing"
    })))
}

/// Get the current sync status
pub async fn get_sync_status(
    state: Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    // Get sync state for INBOX
    match state.cache_service.get_sync_state("INBOX").await {
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
pub async fn get_cached_emails(
    state: Data<DashboardState>,
    query: web::Query<serde_json::Value>,
) -> Result<impl Responder, ApiError> {
    let folder = query.get("folder")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let limit = query.get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(50);

    let offset = query.get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0);

    info!("Getting cached emails for folder: {}, limit: {}, offset: {}", folder, limit, offset);

    match state.cache_service.get_cached_emails(folder, limit, offset).await {
        Ok(emails) => {
            info!("Retrieved {} cached emails", emails.len());
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "emails": emails,
                "folder": folder,
                "count": emails.len(),
            })))
        }
        Err(e) => {
            error!("Failed to get cached emails: {}", e);
            Err(ApiError::InternalError(format!("Failed to get cached emails: {}", e)))
        }
    }
}
