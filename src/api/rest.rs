use actix_web::{web, App, HttpServer, Responder, HttpResponse, ResponseError, HttpRequest};
use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use log::{self, warn, error}; // Removed info, debug
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc; // Removed Mutex
use std::collections::HashMap;
use crate::imap::types::SearchCriteria;
use crate::imap::error::ImapError;
use thiserror::Error; // For defining API-specific errors
use crate::config::RestConfig;
use crate::mcp_port::McpTool;
use crate::imap::client::ImapClient;
use crate::imap::types::{ModifyFlagsPayload, AppendEmailPayload, MailboxInfo}; // Added MailboxInfo
use urlencoding;
use tokio::sync::Mutex as TokioMutex; // Use Tokio Mutex
use crate::api::mcp_sse::{SseState, SseAdapter};
use crate::prelude::ImapSession;

// --- API Specific Error Handling ---

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    #[error("IMAP operation failed")]
    ImapOperationFailed(#[source] ImapError),
}

pub async fn get_raw_email(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> Result<HttpResponse, ApiError> {
    let (encoded_folder, uid) = path.into_inner();
    let folder_name = urlencoding::decode(&encoded_folder)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();

    log::info!("Fetching raw email UID {} from folder '{}'", uid, folder_name);

    // Select the folder
    state.imap_client.select_folder(&folder_name).await?;

    // Fetch raw RFC822 message
    let raw_message = state.imap_client.fetch_raw_message(uid).await?;

    Ok(HttpResponse::Ok()
        .content_type("message/rfc822")
        .body(raw_message))
}

// Manual implementation to avoid issues with #[from] on non-Clone ImapError if it were not Clone
// Also allows for more specific mapping logic.
impl From<ImapError> for ApiError {
    fn from(err: ImapError) -> Self {
        warn!("Converting ImapError to ApiError: {:?}", err);
         match err {
            ImapError::Auth(_) => ApiError::Unauthorized("Authentication failed".to_string()), // More specific public message
            ImapError::Mailbox(msg) | ImapError::Operation(msg) if msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("doesn't exist") => ApiError::NotFound(msg),
            ImapError::Command(msg) | ImapError::Parse(msg) => ApiError::BadRequest(msg),
            ImapError::Connection(msg) | ImapError::Tls(msg) => ApiError::InternalError(format!("IMAP Connection Error: {}", msg)),
             _ => {
                 error!("Unhandled IMAP Error resulted in 500: {:?}", err);
                 ApiError::InternalError("An unexpected IMAP error occurred".to_string())
             }
         }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ImapOperationFailed(ref source_err) => match source_err {
                ImapError::Auth(_) => StatusCode::UNAUTHORIZED,
                ImapError::Mailbox(_) => StatusCode::NOT_FOUND,
                ImapError::Command(_) => StatusCode::BAD_REQUEST,
                ImapError::Parse(_) => StatusCode::BAD_REQUEST,
                 _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let message = self.to_string();
        // Log internal errors with more detail if possible
        if status.is_server_error() {
             error!("Responding with status {} and message: {}", status, message);
        }
        HttpResponse::build(status).json(json!({ "error": message }))
    }
}

// Helper to convert string UIDs to Vec<u32>
fn parse_uids(uids_str: &str) -> Result<Vec<u32>, ApiError> {
    uids_str
        .split(',')
        .map(|s| {
            s.trim().parse::<u32>().map_err(|_| {
                ApiError::BadRequest(format!("Invalid UID format: {}", s))
            })
        })
        .collect()
}

// --- Configuration and State ---

// Application state shared across handlers
#[derive(Clone)] // Ensure AppState can be cloned for Actix data sharing
pub struct AppState {
    pub imap_client: Arc<ImapClient>,
    pub tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>,
    pub sessions: Arc<TokioMutex<HashMap<String, SessionState>>>,
}

impl AppState {
    pub fn new(imap_client: Arc<ImapClient>, tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>) -> Self {
        Self {
            imap_client,
            tool_registry,
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Debug, Default)] // Add Clone, Debug, Default derive
pub struct SessionState {
    pub selected_folder: Option<String>,
}

// --- Route Handlers ---

// GET /api/v1/health
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "OK" }))
}

// GET /folders
pub async fn list_folders(state: web::Data<AppState>) -> Result<impl Responder, ApiError> {
    log::debug!("Handling GET /folders");
    let folders = state.imap_client.list_folders().await?;
    Ok(HttpResponse::Ok().json(folders))
}

// POST /folders { "name": "folder_name" }
#[derive(Deserialize, Debug)]
pub struct FolderCreatePayload { pub name: String }

pub async fn create_folder(
    state: web::Data<AppState>,
    payload: web::Json<FolderCreatePayload>,
) -> Result<impl Responder, ApiError> {
    let folder_name = payload.name.trim();
    log::info!("Handling POST /folders with name: {}", folder_name);
    if folder_name.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty".to_string()));
    }
    // Prepend "INBOX." if it's not "INBOX" itself
    let full_folder_name = if folder_name.eq_ignore_ascii_case("INBOX") {
        folder_name.to_string()
    } else {
        format!("INBOX.{}", folder_name)
    };

    log::debug!("Attempting to create folder: {}", full_folder_name);
    state.imap_client.create_folder(&full_folder_name).await?;
    log::info!("Folder '{}' created successfully.", full_folder_name);
    Ok(HttpResponse::Created().json(json!({ "message": "Folder created", "name": full_folder_name })))
}

// DELETE /folders/{folder_name}
pub async fn delete_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name_base = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();

    log::info!("Handling DELETE /folders/{} (decoded: {})", encoded_folder_name, folder_name_base);
    if folder_name_base.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }
    // Prepend "INBOX." if it's not "INBOX" itself
    let full_folder_name = if folder_name_base.eq_ignore_ascii_case("INBOX") {
        folder_name_base // Use "INBOX" directly
    } else {
        format!("INBOX.{}", folder_name_base)
    };

    log::debug!("Attempting to delete folder: {}", full_folder_name);
    state.imap_client.delete_folder(&full_folder_name).await?;
    log::info!("Folder '{}' deleted successfully.", full_folder_name);
    Ok(HttpResponse::Ok().json(json!({ "message": "Folder deleted", "name": full_folder_name })))
}

// PUT /folders/{from_name} { "to_name": "new_name" }
#[derive(Deserialize, Debug)]
pub struct FolderRenamePayload { pub to_name: String }

pub async fn rename_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<FolderRenamePayload>,
) -> Result<impl Responder, ApiError> {
    let encoded_from_name = path.into_inner();
    let from_name_base = urlencoding::decode(&encoded_from_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid 'from' folder name encoding: {}", e)))?
        .into_owned();
    let to_name_base = payload.to_name.trim();

    log::info!(
        "Handling PUT /folders/{} (decoded: {}) to rename to: {}",
        encoded_from_name, from_name_base, to_name_base
    );

    if from_name_base.is_empty() || to_name_base.is_empty() {
        return Err(ApiError::BadRequest("Folder names cannot be empty".to_string()));
    }

    // Handle INBOX prefixing for both names
    let full_from_name = if from_name_base.eq_ignore_ascii_case("INBOX") {
        from_name_base
    } else {
        format!("INBOX.{}", from_name_base)
    };
    let full_to_name = if to_name_base.eq_ignore_ascii_case("INBOX") {
        to_name_base.to_string() // Ensure it's owned String
    } else {
        format!("INBOX.{}", to_name_base)
    };

    log::debug!("Attempting to rename folder '{}' to '{}'", full_from_name, full_to_name);
    state.imap_client.rename_folder(&full_from_name, &full_to_name).await?;
    log::info!("Folder '{}' renamed to '{}' successfully.", full_from_name, full_to_name);
    Ok(HttpResponse::Ok().json(json!({ 
        "message": "Folder renamed", 
        "from_name": full_from_name, 
        "to_name": full_to_name 
    })))
}

// POST /folders/{folder_name}/select
async fn select_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name_base = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();

    // Log decoded name
    log::info!("Handling POST /folders/{}/select (decoded: {})", encoded_folder_name, folder_name_base);

    if folder_name_base.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }

    let full_folder_name = if folder_name_base.eq_ignore_ascii_case("INBOX") {
        folder_name_base
    } else {
        format!("INBOX.{}", folder_name_base)
    };

    log::debug!("Attempting to select folder: {}", full_folder_name);
    // Add explicit type MailboxInfo
    let mailbox_info: MailboxInfo = state.imap_client.select_folder(&full_folder_name).await?;
    log::info!("Folder '{}' selected successfully.", full_folder_name);

    // --- Session State Update ---
    let session_id = req.headers().get("X-Session-ID").map_or_else(
        || "default_session".to_string(),
        |h| h.to_str().unwrap_or("default_session").to_string()
    ); 
    
    let mut sessions = state.sessions.lock().await;
    let session_state = sessions.entry(session_id.clone()).or_insert_with(SessionState::default);
    session_state.selected_folder = Some(full_folder_name.clone());
    log::debug!("Session '{}' updated: selected folder set to '{}'", session_id, full_folder_name);
    // --- End Session State Update ---

    Ok(HttpResponse::Ok().json(mailbox_info))
}

// Updated search_emails to handle more query params
#[derive(Deserialize, Debug)]
struct EmailSearchQuery {
    subject: Option<String>,
    from: Option<String>,
    to: Option<String>, // Added TO
    body: Option<String>,
    since: Option<String>, // Added SINCE
    uid: Option<String>,   // Added UID set
    unseen: Option<String>,
    flagged: Option<String>, // Added FLAGGED search
    // Add other criteria as needed, potentially moving to POST with JSON body for complex AND/OR/NOT
}

async fn search_emails(
    state: web::Data<AppState>,
    query: web::Query<EmailSearchQuery>,
) -> Result<impl Responder, ApiError> {
    log::info!("Handling GET /emails/search with query: {:?}", query);
    
    // Build SearchCriteria list from query params
    let mut criteria_list = Vec::new();
    if let Some(s) = &query.subject { criteria_list.push(SearchCriteria::Subject(s.clone())); }
    if let Some(s) = &query.from { criteria_list.push(SearchCriteria::From(s.clone())); }
    if let Some(s) = &query.to { criteria_list.push(SearchCriteria::To(s.clone())); }
    if let Some(s) = &query.body { criteria_list.push(SearchCriteria::Body(s.clone())); }
    if let Some(s) = &query.since { criteria_list.push(SearchCriteria::Since(s.clone())); }
    if let Some(s) = &query.uid { criteria_list.push(SearchCriteria::Uid(s.clone())); }
    if query.unseen.is_some() { criteria_list.push(SearchCriteria::Unseen); }
    if query.flagged.is_some() { criteria_list.push(SearchCriteria::Flagged); } // Handle flagged
    
    // Combine criteria using AND if multiple are provided, else use single or ALL
    let final_criteria = match criteria_list.len() {
        0 => SearchCriteria::All,
        1 => criteria_list.remove(0),
        _ => SearchCriteria::And(criteria_list),
    };

    log::debug!("Constructed search criteria: {:?}", final_criteria);
    let uids = state.imap_client.search_emails(final_criteria).await?;
    Ok(HttpResponse::Ok().json(json!({ "uids": uids })))
}

// Updated fetch_emails to accept optional body flag
#[derive(Deserialize, Debug)]
struct EmailFetchQuery {
    uids: String,
    body: Option<String>, // Accept `?body=true` or similar
}

async fn fetch_emails(
    state: web::Data<AppState>,
    query: web::Query<EmailFetchQuery>,
) -> Result<impl Responder, ApiError> {
    let uids = parse_uids(&query.uids)?;
    if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided in query string".to_string()));
    }
    // Check the body query param
    let fetch_body = query.body.as_deref().map_or(false, |b| b.eq_ignore_ascii_case("true"));
    let emails = state.imap_client.fetch_emails(uids, fetch_body).await?;
    Ok(HttpResponse::Ok().json(emails))
}

// POST /emails/move { "uids": [1, 2], "destination_folder": "Archive" }
#[derive(Deserialize, Debug)]
pub struct EmailMovePayload {
    uids: Vec<u32>,
    destination_folder: String,
}

pub async fn move_emails(
    imap_session_trait: web::Data<Arc<dyn ImapSession>>, // Renamed to avoid conflict
    app_state: web::Data<AppState>, // Get AppState for sessions
    payload: web::Json<EmailMovePayload>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    log::info!("Handling POST /emails/move with payload: {:?}", payload);

    // Access sessions via AppState
    // Use the concrete type from AppState
    let session_map: &Arc<TokioMutex<HashMap<String, SessionState>>> = &app_state.sessions; 

    let session_id = req.cookie("session_id")
        .ok_or_else(|| {
            log::warn!("Missing session_id cookie in /emails/move request");
            ApiError::Unauthorized("Missing session information".to_string())
        })?
        .value()
        .to_string();

    let source_folder = {
        let sessions = session_map.lock().await; // Lock the map from AppState
        let current_session_state = sessions.get(&session_id).ok_or_else(|| {
            log::warn!("Session state not found for session_id: {}", session_id);
            ApiError::Unauthorized("Invalid or expired session".to_string())
        })?;
        current_session_state.selected_folder.clone().ok_or_else(|| {
             log::warn!("No folder selected for session_id: {} before move operation", session_id);
             ApiError::BadRequest("No folder currently selected. Please select a folder first.".to_string())
        })?
    };

    log::debug!("Move request for session {}, source folder: '{}', destination: '{}', UIDs: {:?}",
               session_id, source_folder, payload.destination_folder, payload.uids);

    let imap_session = imap_session_trait.get_ref(); // Use the renamed Data<> variable

    // Call the updated move_email function with the source folder
    imap_session.move_email(&source_folder, payload.uids.clone(), &payload.destination_folder).await?;

    let response_msg = format!(
        "Successfully moved {} email(s) from '{}' to '{}'",
        payload.uids.len(),
        source_folder,
        payload.destination_folder
    );
    Ok(HttpResponse::Ok().json(json!({ "message": response_msg })))
}

// POST /folders/{folder_name}/emails - Appends an email
pub async fn append_email(
    app_state: web::Data<AppState>,
    folder_name_encoded: web::Path<String>,
    payload: web::Json<AppendEmailPayload>,
) -> Result<HttpResponse, ApiError> {
    // Log before moving folder_name_encoded
    log::info!("Handling POST /folders/.../emails/append request..."); 
    let folder_name_base = urlencoding::decode(&folder_name_encoded.into_inner())
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    
    // Log decoded name and payload
    log::info!("Appending email to folder (decoded: {}) with payload: {:?}", 
         folder_name_base, payload);

    if folder_name_base.is_empty() {
         return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }

    // Determine full folder name
    let full_folder_name = if folder_name_base.eq_ignore_ascii_case("INBOX") {
        folder_name_base
    } else {
        format!("INBOX.{}", folder_name_base)
    };

    // The payload is already AppendEmailPayload, no need to decode base64 here
    // It's assumed ImapClient::append handles the payload structure correctly
    log::debug!("Attempting to append email to folder '{}' with flags: {:?}", 
        full_folder_name, payload.flags);

    // Perform append operation using the received payload directly
    app_state.imap_client.append(&full_folder_name, payload.into_inner()).await?;

    // Remove the redundant flag storing logic, assume append handles it
    /*
    if !flags.is_empty() {
        // ... existing flag storing logic ...
    }
    */

    log::info!("Email appended successfully to folder '{}'.", full_folder_name);
    Ok(HttpResponse::Ok().json(json!({ 
        "message": "Email appended successfully", 
        "folder": full_folder_name 
    })))
}

// POST /emails/flags - Modifies flags
pub async fn modify_flags(
    app_state: web::Data<AppState>,
    payload: web::Json<ModifyFlagsPayload>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    // --- Get selected folder from session state ---
    let session_id = req.cookie("session_id")
        .ok_or_else(|| {
            log::warn!("Missing session_id cookie in /emails/flags request");
            ApiError::Unauthorized("Missing session information".to_string())
        })?
        .value()
        .to_string();

    let selected_folder = {
        let sessions = app_state.sessions.lock().await;
        sessions.get(&session_id)
            .ok_or_else(|| ApiError::Unauthorized("Invalid or expired session".to_string()))?
            .selected_folder.clone()
            .ok_or_else(|| {
                 log::warn!("Modify flags attempt failed: No folder selected in session '{}'", session_id);
                 ApiError::BadRequest("No folder selected. Please select a folder first.".to_string())
            })?
    };
    log::info!("Handling POST /emails/flags in folder '{}' for session '{}'", selected_folder, session_id);
    // --- End Session State Check ---

    let uids = &payload.uids;
    // Clone the operation from the payload
    let operation = payload.operation.clone(); 
    let flags = &payload.flags;

    if uids.is_empty() {
        return Err(ApiError::BadRequest("UID list cannot be empty".to_string()));
    }
    if flags.items.is_empty() {
         return Err(ApiError::BadRequest("Flags list cannot be empty".to_string()));
    }

    log::debug!(
        "Attempting to {:?} flags {:?} for UIDs {:?} in folder '{}'",
        operation, flags, uids, selected_folder
    );

    // Clone operation again when passing to store_flags
    app_state.imap_client.store_flags(uids.clone(), operation.clone(), flags.clone()).await?;
    
    // Log uses the original operation variable (which is a clone)
    log::info!("Flags {:?} {:?} successfully for UIDs {:?} in folder '{}'", 
        operation, flags, uids, selected_folder);
    Ok(HttpResponse::Ok().json(json!({ 
        "message": "Flags modified successfully", 
        "uids": uids, 
        "operation": operation,
        "flags": flags,
        "folder": selected_folder
    })))
}

// POST /folders/{folder_name}/expunge
async fn expunge_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name_base = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling POST /folders/{}/expunge (decoded: {})", encoded_folder_name, folder_name_base);
    // Note: Expunge operates on the *currently selected* folder in the session.
    // We might want to add a select call here or document this requirement clearly.
    // For now, just call expunge, assuming select was done prior.
    let response = state.imap_client.expunge().await?;
    Ok(HttpResponse::Ok().json(response))
}

// Handler for searching within a specific folder
async fn search_emails_in_folder(
    state: web::Data<AppState>,
    path: web::Path<String>, // Contains the URL-encoded folder name
    query: web::Query<EmailSearchQuery>, // Reuse the same query structure
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name_base = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();

    log::info!(
        "Handling GET /folders/{}/emails/search (decoded: {}) with query: {:?}",
        encoded_folder_name,
        folder_name_base,
        query
    );

    if folder_name_base.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }

    // Prepend INBOX. prefix only if the base name is not INBOX
    let full_folder_name = if folder_name_base.eq_ignore_ascii_case("INBOX") {
        folder_name_base // Use "INBOX" directly
    } else {
        format!("INBOX.{}", folder_name_base)
    };

    // --- Select the folder first --- 
    log::debug!("Selecting folder '{}' before searching...", full_folder_name);
    match state.imap_client.select_folder(&full_folder_name).await {
        Ok(_) => log::debug!("Folder '{}' selected successfully.", full_folder_name),
        Err(e) => {
            log::error!("Failed to select folder '{}' for search: {:?}", full_folder_name, e);
            // Convert IMAP error to API error. Could be NotFound if folder doesn't exist.
            return Err(e.into()); 
        }
    }

    // --- Build Search Criteria (same logic as global search) ---
    let mut criteria_list = Vec::new();
    if let Some(s) = &query.subject { criteria_list.push(SearchCriteria::Subject(s.clone())); }
    if let Some(s) = &query.from { criteria_list.push(SearchCriteria::From(s.clone())); }
    if let Some(s) = &query.to { criteria_list.push(SearchCriteria::To(s.clone())); }
    if let Some(s) = &query.body { criteria_list.push(SearchCriteria::Body(s.clone())); }
    if let Some(s) = &query.since { criteria_list.push(SearchCriteria::Since(s.clone())); }
    if let Some(s) = &query.uid { criteria_list.push(SearchCriteria::Uid(s.clone())); }
    if query.unseen.is_some() { criteria_list.push(SearchCriteria::Unseen); }
    if query.flagged.is_some() { criteria_list.push(SearchCriteria::Flagged); } // Handle flagged
    
    let final_criteria = match criteria_list.len() {
        0 => SearchCriteria::All, // Default to ALL if no specific query params
        1 => criteria_list.remove(0),
        _ => SearchCriteria::And(criteria_list), // AND multiple criteria
    };

    // --- Perform the search --- 
    log::debug!("Performing search in folder '{}' with criteria: {:?}", full_folder_name, final_criteria);
    match state.imap_client.search_emails(final_criteria).await {
        Ok(uids) => {
            log::info!("Search in '{}' successful, found {} UIDs.", full_folder_name, uids.len());
            // Return the Vec<u32> directly as the JSON body
            Ok(HttpResponse::Ok().json(uids))
        }
        Err(e) => {
            log::error!("IMAP search failed in folder '{}': {:?}", full_folder_name, e);
            Err(e.into())
        }
    }
}

// Handler for fetching emails within a specific folder
async fn fetch_emails_in_folder(
    state: web::Data<AppState>,
    path: web::Path<String>, // Contains the URL-encoded folder name
    query: web::Query<EmailFetchQuery>, // Reuse the same query structure
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name_base = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();

    log::info!(
        "Handling GET /folders/{}/emails/fetch (decoded: {}) with query: {:?}",
        encoded_folder_name,
        folder_name_base,
        query
    );

    if folder_name_base.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }

    // Parse UIDs
    let uids = parse_uids(&query.uids)?;
    if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided in query string".to_string()));
    }
    // Check the body query param
    let fetch_body = query.body.as_deref().map_or(false, |b| b.eq_ignore_ascii_case("true"));

    // Determine full folder name (handle INBOX case)
    let full_folder_name = if folder_name_base.eq_ignore_ascii_case("INBOX") {
        folder_name_base
    } else {
        format!("INBOX.{}", folder_name_base)
    };

    // --- Select the folder first --- 
    log::debug!("Selecting folder '{}' before fetching...", full_folder_name);
    match state.imap_client.select_folder(&full_folder_name).await {
        Ok(_) => log::debug!("Folder '{}' selected successfully for fetch.", full_folder_name),
        Err(e) => {
            log::error!("Failed to select folder '{}' for fetch: {:?}", full_folder_name, e);
            return Err(e.into()); 
        }
    }

    // --- Fetch the emails --- 
    log::debug!("Fetching emails from '{}' with UIDs {:?} (body: {})", full_folder_name, uids, fetch_body);
    let emails = state.imap_client.fetch_emails(uids, fetch_body).await?;
    log::info!("Fetch from '{}' successful, got {} emails.", full_folder_name, emails.len());
    Ok(HttpResponse::Ok().json(emails))
}

// --- Service Configuration (Updated) ---

pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
    log::info!("Configuring REST API routes under /api/v1");
    cfg.service(
        web::scope("/api/v1")
            .route("/health", web::get().to(health_check))
            .service(
                web::scope("/folders")
                    .route("", web::get().to(list_folders))
                    .route("", web::post().to(create_folder))
                    .route("/{folder_name}", web::delete().to(delete_folder))
                    .route("/{folder_name}", web::put().to(rename_folder))
                    .route("/{folder_name}/select", web::post().to(select_folder))
                    // Add append and expunge routes under specific folder
                    .route("/{folder_name}/emails/append", web::post().to(append_email))
                    .route("/{folder_name}/expunge", web::post().to(expunge_folder))
                    .route("/{folder_name}/emails/search", web::get().to(search_emails_in_folder))
                    .route("/{folder_name}/emails/{uid}/raw", web::get().to(get_raw_email))
                    // Add route for modifying flags within a folder
                    .route("/{folder_name}/emails/flags", web::post().to(modify_flags))
                    // Add route for fetching emails within a folder
                    .route("/{folder_name}/emails/fetch", web::get().to(fetch_emails_in_folder))
            )
            .service(
                web::scope("/emails")
                    .route("/search", web::get().to(search_emails))
                    // Keep global fetch? Maybe remove if folder-specific is always preferred?
                    .route("/fetch", web::get().to(fetch_emails)) 
                    .route("/move", web::post().to(move_emails))
            )
    );
}

// --- Server Initialization ---

pub async fn run_server(
    config: RestConfig,
    imap_client: Arc<ImapClient>, // Changed to Arc<ImapClient>
    tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>, // Pass the Arc registry
    sse_state: Arc<TokioMutex<SseState>>, // Pass SseState here
  ) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", config.host, config.port);
    log::info!("Starting REST API server at {}", bind_address);

    // Create the application state using the passed-in Arcs
    let app_state = AppState::new(imap_client, tool_registry);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone())) // Share state (contains client and registry)
            .app_data(web::Data::new(sse_state.clone())) // Share SSE state
            .wrap(Logger::default()) // Basic request logging
            .configure(configure_rest_service) // Register routes
            .configure(SseAdapter::configure_sse_service) // Register SSE routes
    })
    .bind(&bind_address)?
    .run()
    .await
}
