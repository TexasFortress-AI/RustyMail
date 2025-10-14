use std::sync::Arc;
use std::path::PathBuf;
use serde_json::{Value, json};
use tokio::sync::Mutex as TokioMutex;
use log::{info, warn, debug, error};
use crate::mcp::types::{JsonRpcError, McpPortState};
use crate::prelude::AsyncImapOps;
use crate::dashboard::services::attachment_storage::{self, AttachmentInfo};

/// Tool for listing attachments for a specific email
///
/// Parameters:
/// - folder: (required if no message_id) The folder name
/// - uid: (required if no message_id) The email UID
/// - message_id: (optional) The message ID (alternative to folder+uid)
/// - account_id: (required) The account email address
pub async fn list_email_attachments_tool(
    session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    // Get cache service from state and clone the Arc
    let cache_service_arc = {
        let state_guard = state.lock().await;
        state_guard.cache_service.as_ref()
            .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?
            .clone()
    };

    let db_pool = cache_service_arc.db_pool.as_ref()
        .ok_or_else(|| JsonRpcError::internal_error("Database pool not available"))?;

    // Extract account_id
    let account_id = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    // Get message_id - either directly provided or resolve from folder+uid
    let message_id = if let Some(msg_id) = params.get("message_id").and_then(|v| v.as_str()) {
        msg_id.to_string()
    } else {
        // Resolve from folder + uid
        let folder = params.get("folder")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("folder parameter is required when message_id is not provided"))?;

        let uid = params.get("uid")
            .and_then(|v| v.as_u64())
            .map(|u| u as u32)
            .ok_or_else(|| JsonRpcError::invalid_params("uid parameter is required when message_id is not provided"))?;

        // Fetch email to get message_id
        session.select_folder(folder).await.map_err(|e| {
            JsonRpcError::internal_error(format!("Failed to select folder: {}", e))
        })?;

        let emails = session.fetch_emails(&[uid]).await.map_err(|e| {
            JsonRpcError::internal_error(format!("Failed to fetch email: {}", e))
        })?;

        let email = emails.into_iter().next()
            .ok_or_else(|| JsonRpcError::invalid_params(format!("Email with UID {} not found", uid)))?;

        attachment_storage::ensure_message_id(&email, account_id)
    };

    // Get attachments metadata from database
    let attachments = attachment_storage::get_attachments_metadata(db_pool, account_id, &message_id)
        .await
        .map_err(|e| JsonRpcError::internal_error(format!("Failed to get attachments: {}", e)))?;

    info!("Listed {} attachments for message_id: {}", attachments.len(), message_id);

    Ok(json!({
        "success": true,
        "message_id": message_id,
        "account_id": account_id,
        "attachments": attachments,
        "count": attachments.len()
    }))
}

/// Tool for downloading attachments for a specific email to a local directory
///
/// Parameters:
/// - folder: (required if no message_id) The folder name
/// - uid: (required if no message_id) The email UID
/// - message_id: (optional) The message ID (alternative to folder+uid)
/// - account_id: (required) The account email address
/// - destination: (optional) Destination directory. Defaults to ~/Downloads/rustymail_attachments/{account}/{message_id}/
/// - create_zip: (optional) If true, create a ZIP archive instead of individual files
pub async fn download_email_attachments_tool(
    session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    // Get cache service from state and clone the Arc
    let cache_service_arc = {
        let state_guard = state.lock().await;
        state_guard.cache_service.as_ref()
            .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?
            .clone()
    };

    let db_pool = cache_service_arc.db_pool.as_ref()
        .ok_or_else(|| JsonRpcError::internal_error("Database pool not available"))?;

    // Extract account_id
    let account_id = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    // Get message_id - either directly provided or resolve from folder+uid
    let message_id = if let Some(msg_id) = params.get("message_id").and_then(|v| v.as_str()) {
        msg_id.to_string()
    } else {
        // Resolve from folder + uid
        let folder = params.get("folder")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("folder parameter is required when message_id is not provided"))?;

        let uid = params.get("uid")
            .and_then(|v| v.as_u64())
            .map(|u| u as u32)
            .ok_or_else(|| JsonRpcError::invalid_params("uid parameter is required when message_id is not provided"))?;

        // Fetch email to get message_id
        session.select_folder(folder).await.map_err(|e| {
            JsonRpcError::internal_error(format!("Failed to select folder: {}", e))
        })?;

        let emails = session.fetch_emails(&[uid]).await.map_err(|e| {
            JsonRpcError::internal_error(format!("Failed to fetch email: {}", e))
        })?;

        let email = emails.into_iter().next()
            .ok_or_else(|| JsonRpcError::invalid_params(format!("Email with UID {} not found", uid)))?;

        attachment_storage::ensure_message_id(&email, account_id)
    };

    // Determine destination directory
    let destination = if let Some(dest) = params.get("destination").and_then(|v| v.as_str()) {
        PathBuf::from(dest)
    } else {
        // Default: ~/Downloads/rustymail_attachments/{account}/{message_id}/
        let home_dir = dirs::home_dir()
            .ok_or_else(|| JsonRpcError::internal_error("Failed to determine home directory"))?;
        let sanitized_message_id = attachment_storage::sanitize_message_id(&message_id);
        home_dir
            .join("Downloads")
            .join("rustymail_attachments")
            .join(account_id)
            .join(sanitized_message_id)
    };

    // Check if ZIP archive is requested
    let create_zip = params.get("create_zip")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if create_zip {
        // Create ZIP archive
        let zip_path = destination.with_extension("zip");
        let result_path = attachment_storage::create_zip_archive(db_pool, account_id, &message_id, &zip_path)
            .await
            .map_err(|e| JsonRpcError::internal_error(format!("Failed to create ZIP archive: {}", e)))?;

        info!("Created ZIP archive at: {:?}", result_path);

        Ok(json!({
            "success": true,
            "message": format!("ZIP archive created successfully"),
            "message_id": message_id,
            "account_id": account_id,
            "destination": result_path.to_string_lossy(),
            "format": "zip"
        }))
    } else {
        // Copy individual attachment files to destination
        let attachments = attachment_storage::get_attachments_metadata(db_pool, account_id, &message_id)
            .await
            .map_err(|e| JsonRpcError::internal_error(format!("Failed to get attachments: {}", e)))?;

        if attachments.is_empty() {
            return Err(JsonRpcError::invalid_params("No attachments found for this email"));
        }

        // Create destination directory
        std::fs::create_dir_all(&destination)
            .map_err(|e| JsonRpcError::internal_error(format!("Failed to create destination directory: {}", e)))?;

        let mut copied_files = Vec::new();
        for attachment in &attachments {
            let source_path = PathBuf::from(&attachment.storage_path);
            let dest_file = destination.join(&attachment.filename);

            if source_path.exists() {
                std::fs::copy(&source_path, &dest_file)
                    .map_err(|e| JsonRpcError::internal_error(format!("Failed to copy {}: {}", attachment.filename, e)))?;

                debug!("Copied attachment: {} -> {:?}", attachment.filename, dest_file);
                copied_files.push(dest_file.to_string_lossy().to_string());
            } else {
                warn!("Attachment file not found: {:?}", source_path);
            }
        }

        info!("Copied {} attachments to: {:?}", copied_files.len(), destination);

        Ok(json!({
            "success": true,
            "message": format!("Copied {} attachments successfully", copied_files.len()),
            "message_id": message_id,
            "account_id": account_id,
            "destination": destination.to_string_lossy(),
            "copied_files": copied_files,
            "count": copied_files.len()
        }))
    }
}

/// Tool for deleting attachments for a specific email
/// This is called when an email is deleted to clean up associated attachments
///
/// Parameters:
/// - message_id: (required) The message ID
/// - account_id: (required) The account email address
pub async fn cleanup_attachments_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    // Get cache service from state
    let state_guard = state.lock().await;
    let cache_service = state_guard.cache_service.as_ref()
        .ok_or_else(|| JsonRpcError::internal_error("Cache service not available"))?;

    let db_pool = cache_service.db_pool.as_ref()
        .ok_or_else(|| JsonRpcError::internal_error("Database pool not available"))?;

    // Extract parameters
    let message_id = params.get("message_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("message_id parameter is required"))?;

    let account_id = params.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("account_id parameter is required"))?;

    // Delete attachments
    attachment_storage::delete_attachments_for_email(db_pool, message_id, account_id)
        .await
        .map_err(|e| JsonRpcError::internal_error(format!("Failed to delete attachments: {}", e)))?;

    info!("Cleaned up attachments for message_id: {}, account: {}", message_id, account_id);

    Ok(json!({
        "success": true,
        "message": format!("Attachments deleted successfully"),
        "message_id": message_id,
        "account_id": account_id
    }))
}
