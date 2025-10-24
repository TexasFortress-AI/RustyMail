// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Cache tools for MCP - these work with the cache service instead of IMAP
use std::sync::Arc;
use serde_json::{Value, json};
use tokio::sync::Mutex as TokioMutex;
use crate::mcp::types::{JsonRpcError, McpPortState};
use crate::dashboard::services::cache::CacheService;
use log::{debug, error};
use crate::prelude::AsyncImapOps;

// Helper function to get cache service from state
async fn get_cache_service(state: &TokioMutex<McpPortState>) -> Option<Arc<CacheService>> {
    let guard = state.lock().await;
    guard.cache_service.clone()
}

/// Tool for listing cached emails from the database
pub async fn list_cached_emails_tool(
    _session: Arc<dyn AsyncImapOps>,  // Not used for cache tools
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing list_cached_emails tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    // Extract parameters
    let (folder, limit, offset, preview_mode, account_email) = if let Some(ref p) = params {
        let folder = p.get("folder")
            .and_then(|v| v.as_str())
            .unwrap_or("INBOX");
        let limit = p.get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(20);
        let offset = p.get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0);
        let preview_mode = p.get("preview_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);  // Default to preview mode for token efficiency
        let account_email = p.get("account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (folder, limit, offset, preview_mode, account_email)
    } else {
        ("INBOX", 20, 0, true, None)
    };

    // Require account_id parameter - no defaults
    let account_id = account_email.as_deref()
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    match cache_service.get_cached_emails_for_account(folder, account_id, limit, offset, preview_mode).await {
        Ok(emails) => {
            Ok(json!({
                "success": true,
                "data": emails,
                "folder": folder,
                "count": emails.len(),
                "tool": "list_cached_emails"
            }))
        }
        Err(e) => {
            error!("Failed to get cached emails: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to get cached emails: {}", e)))
        }
    }
}

/// Tool for getting a cached email by UID
pub async fn get_email_by_uid_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing get_email_by_uid tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    let folder = params.get("folder")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let uid = params.get("uid")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| JsonRpcError::invalid_params("uid parameter is required"))?;

    let account_email = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    match cache_service.get_email_by_uid_for_account(folder, uid, account_email).await {
        Ok(Some(email)) => {
            Ok(json!({
                "success": true,
                "data": email,
                "tool": "get_email_by_uid"
            }))
        }
        Ok(None) => {
            Ok(json!({
                "success": false,
                "error": format!("Email with UID {} not found in {}", uid, folder),
                "tool": "get_email_by_uid"
            }))
        }
        Err(e) => {
            error!("Failed to get email by UID: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to get email by UID: {}", e)))
        }
    }
}

/// Tool for getting a cached email by index position
pub async fn get_email_by_index_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing get_email_by_index tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    let folder = params.get("folder")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let index = params.get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or_else(|| JsonRpcError::invalid_params("index parameter is required"))?;

    let account_email = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    // Get emails sorted by date DESC, then select by index
    // Use full content (preview_mode=false) for individual email requests
    match cache_service.get_cached_emails_for_account(folder, account_email, index + 1, index, false).await {
        Ok(emails) if !emails.is_empty() => {
            Ok(json!({
                "success": true,
                "data": emails[0],
                "tool": "get_email_by_index"
            }))
        }
        Ok(_) => {
            Ok(json!({
                "success": false,
                "error": format!("No email at index {} in {}", index, folder),
                "tool": "get_email_by_index"
            }))
        }
        Err(e) => {
            error!("Failed to get email by index: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to get email by index: {}", e)))
        }
    }
}

/// Tool for counting emails in a folder
pub async fn count_emails_in_folder_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing count_emails_in_folder tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let (folder, account_email) = if let Some(ref p) = params {
        let folder = p.get("folder")
            .and_then(|v| v.as_str())
            .unwrap_or("INBOX");
        let account_email = p.get("account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (folder, account_email)
    } else {
        ("INBOX", None)
    };

    // Require account_id parameter - no defaults
    let account_id = account_email.as_deref()
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    match cache_service.count_emails_in_folder_for_account(folder, account_id).await {
        Ok(count) => {
            Ok(json!({
                "success": true,
                "data": {
                    "count": count,
                    "folder": folder
                },
                "tool": "count_emails_in_folder"
            }))
        }
        Err(e) => {
            error!("Failed to count emails: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to count emails: {}", e)))
        }
    }
}

/// Tool for getting folder statistics
pub async fn get_folder_stats_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing get_folder_stats tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    let folder = params.get("folder")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let account_email = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    match cache_service.get_folder_stats_for_account(folder, account_email).await {
        Ok(stats) => {
            Ok(json!({
                "success": true,
                "data": stats,
                "tool": "get_folder_stats"
            }))
        }
        Err(e) => {
            error!("Failed to get folder stats: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to get folder stats: {}", e)))
        }
    }
}

/// Tool for searching within cached emails
pub async fn search_cached_emails_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    debug!("Executing search_cached_emails tool");

    let cache_service = get_cache_service(&state).await
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    let folder = params.get("folder")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let query = params.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("query parameter is required"))?;

    let limit = params.get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20);

    let account_email = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    match cache_service.search_cached_emails_for_account(folder, query, limit, account_email).await {
        Ok(emails) => {
            Ok(json!({
                "success": true,
                "data": emails,
                "query": query,
                "folder": folder,
                "count": emails.len(),
                "tool": "search_cached_emails"
            }))
        }
        Err(e) => {
            error!("Failed to search emails: {}", e);
            Err(JsonRpcError::internal_error(format!("Failed to search emails: {}", e)))
        }
    }
}