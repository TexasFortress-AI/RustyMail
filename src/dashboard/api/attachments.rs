// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{web, HttpResponse, Responder, HttpRequest};
use actix_files::NamedFile;
use serde::{Deserialize, Serialize};
use log::{debug, error, info};
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use crate::dashboard::services::attachment_storage::{self, AttachmentInfo};
use std::path::PathBuf;

/// Query parameters for listing attachments
#[derive(Debug, Deserialize)]
pub struct ListAttachmentsParams {
    pub folder: Option<String>,
    pub uid: Option<u32>,
    pub message_id: Option<String>,
    pub account_id: String,
}

/// Query parameters for downloading attachments
#[derive(Debug, Deserialize)]
pub struct DownloadAttachmentsParams {
    pub folder: Option<String>,
    pub uid: Option<u32>,
    pub message_id: Option<String>,
    pub account_id: String,
    pub as_zip: Option<bool>,
}

/// Path parameters for single attachment download
#[derive(Debug, Deserialize)]
pub struct AttachmentPathParams {
    pub message_id: String,
    pub filename: String,
}

/// Path parameters for inline attachment download by Content-ID
#[derive(Debug, Deserialize)]
pub struct InlineAttachmentPathParams {
    pub message_id: String,
    pub content_id: String,
}

/// Response for listing attachments
#[derive(Debug, Serialize)]
pub struct ListAttachmentsResponse {
    pub success: bool,
    pub message_id: String,
    pub account_id: String,
    pub attachments: Vec<AttachmentInfo>,
    pub count: usize,
}

/// Handler for listing attachments for a specific email
/// GET /api/attachments/list
pub async fn list_attachments(
    query: web::Query<ListAttachmentsParams>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/attachments/list with params: {:?}", query);

    // Get database pool
    let db_pool = state.cache_service.db_pool.as_ref()
        .ok_or_else(|| ApiError::InternalError("Database not available".to_string()))?;

    // Determine message_id - either directly provided or resolve from folder+uid
    let message_id = if let Some(ref msg_id) = query.message_id {
        msg_id.clone()
    } else {
        // Resolve from folder + uid
        let folder = query.folder.as_deref()
            .ok_or_else(|| ApiError::BadRequest("folder parameter required when message_id not provided".to_string()))?;

        let uid = query.uid
            .ok_or_else(|| ApiError::BadRequest("uid parameter required when message_id not provided".to_string()))?;

        // Fetch email to get message_id
        let emails = state.email_service
            .fetch_emails_for_account(folder, &[uid], &query.account_id)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to fetch email: {}", e)))?;

        let email = emails.into_iter().next()
            .ok_or_else(|| ApiError::NotFound(format!("Email with UID {} not found", uid)))?;

        attachment_storage::ensure_message_id(&email, &query.account_id)
    };

    // Get attachments metadata
    let mut attachments = attachment_storage::get_attachments_metadata(
        db_pool,
        &query.account_id,
        &message_id,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to get attachments: {}", e)))?;

    // If no attachments found in database and we have folder+uid, fetch from IMAP
    if let (true, Some(folder), Some(uid)) = (attachments.is_empty(), query.folder.as_ref(), query.uid) {

        debug!("No attachments in database for message_id {}. Fetching from IMAP...", message_id);

        // Fetch email with attachments from IMAP (this will save them to DB)
        match state.email_service.fetch_email_with_attachments(folder, uid, &query.account_id).await {
            Ok((_, attachment_infos)) => {
                // Attachments now saved to database
                debug!("Successfully fetched and saved {} attachments from IMAP", attachment_infos.len());

                // Re-query database to get the saved attachments
                attachments = attachment_storage::get_attachments_metadata(
                    db_pool,
                    &query.account_id,
                    &message_id,
                )
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to get attachments after IMAP fetch: {}", e)))?;
            }
            Err(e) => {
                error!("Failed to fetch attachments from IMAP: {}", e);
                // Continue with empty attachments list rather than failing the request
            }
        }
    }

    info!("Listed {} attachments for message_id: {}", attachments.len(), message_id);

    let response = ListAttachmentsResponse {
        success: true,
        message_id,
        account_id: query.account_id.clone(),
        count: attachments.len(),
        attachments,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Handler for downloading a single attachment
/// GET /api/attachments/{message_id}/{filename}
pub async fn download_attachment(
    path: web::Path<AttachmentPathParams>,
    query: web::Query<serde_json::Value>,
    state: web::Data<DashboardState>,
    _req: HttpRequest,
) -> Result<NamedFile, ApiError> {
    debug!("Handling GET /api/attachments/{}/{}", path.message_id, path.filename);

    // Get account_id from query parameters
    let account_id = query.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("account_id parameter required".to_string()))?;

    // Get database pool
    let db_pool = state.cache_service.db_pool.as_ref()
        .ok_or_else(|| ApiError::InternalError("Database not available".to_string()))?;

    // Get attachment metadata
    let attachments = attachment_storage::get_attachments_metadata(
        db_pool,
        account_id,
        &path.message_id,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to get attachments: {}", e)))?;

    // Find the specific attachment
    let attachment = attachments.iter()
        .find(|a| a.filename == path.filename)
        .ok_or_else(|| ApiError::NotFound(format!("Attachment '{}' not found", path.filename)))?;

    // Get the file path
    let file_path = PathBuf::from(&attachment.storage_path);

    if !file_path.exists() {
        return Err(ApiError::NotFound(format!("Attachment file not found on disk: {}", path.filename)));
    }

    info!("Serving attachment: {} ({})", path.filename, attachment.size_bytes);

    // Serve the file
    NamedFile::open(&file_path)
        .map_err(|e| ApiError::InternalError(format!("Failed to open attachment: {}", e)))
}

/// Handler for downloading all attachments as a ZIP file
/// GET /api/attachments/{message_id}/zip
pub async fn download_attachments_zip(
    path: web::Path<String>,  // message_id
    query: web::Query<serde_json::Value>,
    state: web::Data<DashboardState>,
) -> Result<NamedFile, ApiError> {
    let message_id = path.into_inner();
    debug!("Handling GET /api/attachments/{}/zip", message_id);

    // Get account_id from query parameters
    let account_id = query.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("account_id parameter required".to_string()))?;

    // Get database pool
    let db_pool = state.cache_service.db_pool.as_ref()
        .ok_or_else(|| ApiError::InternalError("Database not available".to_string()))?;

    // Create temporary path for ZIP file
    let temp_dir = std::env::temp_dir();
    let sanitized_message_id = attachment_storage::sanitize_message_id(&message_id);
    let zip_path = temp_dir.join(format!("rustymail_attachments_{}.zip", sanitized_message_id));

    // Create ZIP archive
    let result_path = attachment_storage::create_zip_archive(
        db_pool,
        account_id,
        &message_id,
        &zip_path,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to create ZIP: {}", e)))?;

    info!("Created ZIP archive for message_id: {} at {:?}", message_id, result_path);

    // Serve the ZIP file
    NamedFile::open(&result_path)
        .map_err(|e| ApiError::InternalError(format!("Failed to open ZIP file: {}", e)))
}

/// Handler for downloading an inline attachment by Content-ID
/// GET /api/attachments/{message_id}/inline/{content_id}
/// This is used to serve images referenced by cid: URIs in HTML emails
pub async fn download_inline_attachment(
    path: web::Path<InlineAttachmentPathParams>,
    query: web::Query<serde_json::Value>,
    state: web::Data<DashboardState>,
    _req: HttpRequest,
) -> Result<NamedFile, ApiError> {
    debug!("Handling GET /api/attachments/{}/inline/{}", path.message_id, path.content_id);

    // Get account_id from query parameters
    let account_id = query.get("account_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("account_id parameter required".to_string()))?;

    // Get database pool
    let db_pool = state.cache_service.db_pool.as_ref()
        .ok_or_else(|| ApiError::InternalError("Database not available".to_string()))?;

    // Get attachment by Content-ID
    let attachment = attachment_storage::get_attachment_by_content_id(
        db_pool,
        account_id,
        &path.message_id,
        &path.content_id,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to get attachment: {}", e)))?
    .ok_or_else(|| ApiError::NotFound(format!("Inline attachment with Content-ID '{}' not found", path.content_id)))?;

    // Get the file path
    let file_path = PathBuf::from(&attachment.storage_path);

    if !file_path.exists() {
        return Err(ApiError::NotFound(format!("Inline attachment file not found on disk: {}", path.content_id)));
    }

    info!("Serving inline attachment: {} (Content-ID: {})", attachment.filename, path.content_id);

    // Serve the file
    NamedFile::open(&file_path)
        .map_err(|e| ApiError::InternalError(format!("Failed to open attachment: {}", e)))
}
