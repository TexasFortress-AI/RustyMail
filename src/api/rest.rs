use actix_web::{web, App, HttpServer, Responder, HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use std::sync::Arc;
use thiserror::Error; // For defining API-specific errors
use serde::Deserialize; // Added for email payloads
use crate::imap::error::ImapError as InternalImapError;
use crate::config::RestConfig;
use log::{error, warn};
use serde_json::json; // Make sure json macro is imported
use crate::imap::client::ImapClient;
use crate::imap::types::{MailboxInfo, SearchCriteria, ModifyFlagsPayload, AppendEmailPayload}; // Include SearchCriteria and new payload types
use urlencoding;

// --- API Specific Error Handling ---

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    #[error("IMAP operation failed")]
    ImapOperationFailed(#[source] InternalImapError),
}

// Manual implementation to avoid issues with #[from] on non-Clone InternalImapError if it were not Clone
// Also allows for more specific mapping logic.
impl From<InternalImapError> for ApiError {
    fn from(err: InternalImapError) -> Self {
        warn!("Converting InternalImapError to ApiError: {:?}", err);
        match err {
            InternalImapError::Auth(_) => ApiError::InternalError("Authentication failed internally".to_string()),
            InternalImapError::Mailbox(msg) | InternalImapError::Operation(msg) if msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("doesn't exist") => ApiError::NotFound(msg),
            InternalImapError::Command(msg) | InternalImapError::Parse(msg) => ApiError::BadRequest(msg),
            InternalImapError::Connection(msg) | InternalImapError::Tls(msg) => ApiError::InternalError(msg),
            _ => ApiError::ImapOperationFailed(err),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ImapOperationFailed(ref source_err) => match source_err {
                InternalImapError::Auth(_) => StatusCode::UNAUTHORIZED,
                InternalImapError::Mailbox(_) => StatusCode::NOT_FOUND,
                InternalImapError::Command(_) => StatusCode::BAD_REQUEST,
                InternalImapError::Parse(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let message = match self {
            ApiError::InternalError(_) => "An internal server error occurred.".to_string(),
            ApiError::ImapOperationFailed(ref source_err) if status == StatusCode::INTERNAL_SERVER_ERROR => {
                 error!("Unhandled IMAP Error resulted in 500: {:?}", source_err);
                "An internal server error occurred during IMAP operation.".to_string()
            }
            _ => self.to_string(),
        };

        error!("Responding with status {} and message: {}", status, message);

        HttpResponse::build(status)
            .json(json!({ "error": message }))
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
#[derive(Clone)]
pub struct AppState {
    pub imap_client: Arc<ImapClient>,
}

// Initialize AppState
impl AppState {
    pub fn new(imap_client: Arc<ImapClient>) -> Self {
        AppState { imap_client }
    }
}

// --- Route Handlers ---

// GET /api/v1/health
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "OK" }))
}

// GET /folders
pub async fn list_folders(state: web::Data<AppState>) -> Result<impl Responder, ApiError> {
    log::info!("Handling GET /folders");
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
    let raw_name = payload.name.trim();
    log::info!("Handling POST /folders with raw name: {}", raw_name);
    if raw_name.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty".to_string()));
    }

    // Prepend INBOX. prefix for compatibility with servers like GoDaddy
    let full_name = format!("INBOX.{}", raw_name);
    log::info!("Attempting to create IMAP folder: {}", full_name);
    
    // Await the result first
    let create_result = state.imap_client.create_folder(&full_name).await;

    // Log the result (success or error) AFTER awaiting
    match create_result {
        Ok(_) => {
            log::info!("IMAP create_folder succeeded for '{}'", full_name);
            // Return the original requested name in the success message
            Ok(HttpResponse::Created().json(json!({ "message": format!("Folder '{}' created", raw_name) })))
        }
        Err(e) => {
            log::error!("IMAP create_folder failed for '{}': {:?}", full_name, e);
            Err(e.into()) // Convert to ApiError using the existing From trait
        }
    }
}

// DELETE /folders/{folder_name}
pub async fn delete_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_name = path.into_inner();
    let base_name = urlencoding::decode(&encoded_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling DELETE /folders/{} (decoded: {})", encoded_name, base_name);
    if base_name.is_empty() {
         return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }

    // Prepend INBOX. prefix
    let full_name = format!("INBOX.{}", base_name);
    log::info!("Attempting to delete IMAP folder: {}", full_name);

    // Add logging similar to create_folder
    match state.imap_client.delete_folder(&full_name).await {
        Ok(_) => {
            log::info!("IMAP delete_folder succeeded for '{}'", full_name);
            // Return the original requested name in the success message
            Ok(HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' deleted", base_name) })))
        }
        Err(e) => {
            log::error!("IMAP delete_folder failed for '{}': {:?}", full_name, e);
            Err(e.into())
        }
    }
}

// PUT /folders/{from_name} { "to_name": "new_name" }
#[derive(Deserialize, Debug)]
pub struct FolderRenamePayload { pub to_name: String }

pub async fn rename_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<FolderRenamePayload>,
) -> Result<impl Responder, ApiError> {
    let encoded_from = path.into_inner();
    let from_name_base = urlencoding::decode(&encoded_from)
        .map_err(|e| ApiError::BadRequest(format!("Invalid 'from' name encoding: {}", e)))?
        .into_owned();
    let to_name_base = payload.to_name.trim();

    log::info!(
        "Handling PUT /folders/{} with payload {{ to_name: {} }} (decoded from: {}, decoded to: {})",
        encoded_from,
        payload.to_name, // Log original payload value
        from_name_base,
        to_name_base
    );

    if from_name_base.is_empty() || to_name_base.is_empty() {
        log::warn!("Rename failed: folder name cannot be empty.");
        return Err(ApiError::BadRequest("Folder names cannot be empty".to_string()));
    }

    // Prepend INBOX. prefix
    let from_full_name = format!("INBOX.{}", from_name_base);
    let to_full_name = format!("INBOX.{}", to_name_base);
    log::info!("Attempting to rename IMAP folder from '{}' to '{}'", from_full_name, to_full_name);

    // Add detailed logging for the IMAP call
    match state.imap_client.rename_folder(&from_full_name, &to_full_name).await {
        Ok(_) => {
            log::info!("IMAP rename_folder succeeded for '{}' -> '{}'", from_full_name, to_full_name);
            Ok(HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' renamed to '{}'", from_name_base, to_name_base) })))
        }
        Err(e) => {
            log::error!("IMAP rename_folder failed for '{}' -> '{}': {:?}", from_full_name, to_full_name, e);
            // Consider mapping specific IMAP errors to more specific ApiErrors if possible
            Err(e.into()) 
        }
    }
}

// POST /folders/{folder_name}/select
async fn select_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_name = path.into_inner();
    let name = urlencoding::decode(&encoded_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling POST /folders/{}/select (decoded: {})", encoded_name, name);
     if name.is_empty() {
         return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }
    // Call the updated client method which now returns MailboxInfo
    let mailbox_info: MailboxInfo = state.imap_client.select_folder(&name).await?;
    // Return the serializable MailboxInfo in the JSON response
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
struct EmailMovePayload { uids: Vec<u32>, destination_folder: String }

async fn move_emails(
    state: web::Data<AppState>,
    payload: web::Json<EmailMovePayload>,
) -> Result<impl Responder, ApiError> {
    log::info!("Handling POST /emails/move with payload: {:?}", payload);
    let uids = payload.uids.clone();
    let dest_folder = payload.destination_folder.trim();

    if uids.is_empty() {
        return Err(ApiError::BadRequest("UID list cannot be empty".to_string()));
    }
    if dest_folder.is_empty() {
        return Err(ApiError::BadRequest("Destination folder cannot be empty".to_string()));
    }

    // Note: Assumes source folder is selected.
    state.imap_client.move_email(uids, dest_folder).await?;
    Ok(HttpResponse::Ok()
        .json(json!({ "message": format!("Emails moved to '{}'", dest_folder) })))
}

// POST /emails/flags { "uids": [...], "operation": "Add|Remove|Set", "flags": { "items": ["\\Seen"] } }
async fn modify_email_flags(
    state: web::Data<AppState>,
    payload: web::Json<ModifyFlagsPayload>,
) -> Result<impl Responder, ApiError> {
    log::info!("Handling POST /emails/flags with payload: {:?}", payload);
    if payload.uids.is_empty() {
        return Err(ApiError::BadRequest("UID list cannot be empty".to_string()));
    }
    // Note: Assumes folder is already selected.
    state.imap_client.store_flags(payload.uids.clone(), payload.operation.clone(), payload.flags.clone()).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": "Flags updated successfully" })))
}

// POST /folders/{folder_name}/append { "content": "...", "flags": { "items": [...] } }
async fn append_email(
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<AppendEmailPayload>,
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
     let folder_name = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling POST /folders/{}/append (decoded: {})", encoded_folder_name, folder_name);
    if folder_name.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty".to_string()));
    }
    
    let result = state.imap_client.append(&folder_name, payload.into_inner()).await?;
    // Result might contain the UID of the appended message
    match result {
        Some(uid) => Ok(HttpResponse::Created().json(json!({ "message": "Email appended successfully", "uid": uid }))),
        None => Ok(HttpResponse::Created().json(json!({ "message": "Email appended successfully (UID not provided by server)" }))),
    }
}

// POST /folders/{folder_name}/expunge
async fn expunge_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_folder_name = path.into_inner();
    let folder_name = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling POST /folders/{}/expunge (decoded: {})", encoded_folder_name, folder_name);
    // Note: Expunge operates on the *currently selected* folder in the session.
    // We might want to add a select call here or document this requirement clearly.
    // For now, just call expunge, assuming select was done prior.
    let response = state.imap_client.expunge().await?;
    Ok(HttpResponse::Ok().json(response))
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
                    .route("/{folder_name}/append", web::post().to(append_email))
                    .route("/{folder_name}/expunge", web::post().to(expunge_folder))
            )
            .service(
                web::scope("/emails")
                    .route("/search", web::get().to(search_emails))
                    .route("/fetch", web::get().to(fetch_emails))
                    .route("/move", web::post().to(move_emails))
                    // Add route for modifying flags
                    .route("/flags", web::post().to(modify_email_flags))
            )
    );
}

// --- Server Initialization ---

pub async fn run_server(
    config: RestConfig,
    imap_client: Arc<ImapClient>, // Pass the Arc<ImapClient>
  ) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", config.host, config.port);
    log::info!("Starting REST API server at {}", bind_address);

    // Create the application state
    let app_state = AppState {
        imap_client, // Move the Arc into the state
        // rest_config: config.clone(), // Clone config if needed by handlers
    };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone())) // Share state
            .wrap(Logger::default()) // Basic request logging
            .configure(configure_rest_service) // Register routes
    })
    .bind(&bind_address)?
    .run()
    .await
} 