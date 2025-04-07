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
use crate::imap::types::{Folder, Email, MailboxInfo, SearchCriteria}; // Include SearchCriteria
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
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "OK" }))
}

// GET /folders
async fn list_folders(state: web::Data<AppState>) -> Result<impl Responder, ApiError> {
    log::info!("Handling GET /folders");
    let folders = state.imap_client.list_folders().await?;
    Ok(HttpResponse::Ok().json(folders))
}

// POST /folders { "name": "folder_name" }
#[derive(Deserialize, Debug)]
struct FolderCreatePayload { name: String }

async fn create_folder(
    state: web::Data<AppState>,
    payload: web::Json<FolderCreatePayload>,
) -> Result<impl Responder, ApiError> {
    let name = payload.name.trim();
    log::info!("Handling POST /folders with name: {}", name);
    if name.is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty".to_string()));
    }
    state.imap_client.create_folder(name).await?;
    Ok(HttpResponse::Created().json(json!({ "message": format!("Folder '{}' created", name) })))
}

// DELETE /folders/{folder_name}
async fn delete_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let encoded_name = path.into_inner();
    let name = urlencoding::decode(&encoded_name)
        .map_err(|e| ApiError::BadRequest(format!("Invalid folder name encoding: {}", e)))?
        .into_owned();
    log::info!("Handling DELETE /folders/{} (decoded: {})", encoded_name, name);
    if name.is_empty() {
         return Err(ApiError::BadRequest("Folder name cannot be empty after decoding".to_string()));
    }
    state.imap_client.delete_folder(&name).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' deleted", name) })))
}

// PUT /folders/{from_name} { "to_name": "new_name" }
#[derive(Deserialize, Debug)]
struct FolderRenamePayload { to_name: String }

async fn rename_folder(
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<FolderRenamePayload>,
) -> Result<impl Responder, ApiError> {
    let encoded_from = path.into_inner();
    let from_name = urlencoding::decode(&encoded_from)
        .map_err(|e| ApiError::BadRequest(format!("Invalid 'from' name encoding: {}", e)))?
        .into_owned();

    let to_name = payload.to_name.trim();
    log::info!("Handling PUT /folders/{} with to_name: {} (decoded from: {})", encoded_from, to_name, from_name);

    if from_name.is_empty() || to_name.is_empty() {
         return Err(ApiError::BadRequest("Folder names cannot be empty".to_string()));
    }

    state.imap_client.rename_folder(&from_name, to_name).await?;
    Ok(HttpResponse::Ok()
        .json(json!({ "message": format!("Folder '{}' renamed to '{}'", from_name, to_name) })))
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

// GET /emails/search?criteria=... (Simplified: uses query params)
#[derive(Deserialize, Debug)]
struct EmailSearchQuery {
    subject: Option<String>,
    from: Option<String>,
    body: Option<String>,
    all: Option<String>, // Flag-like param
    unseen: Option<String>, // Flag-like param
    // Add other criteria as needed
}

async fn search_emails(
    state: web::Data<AppState>,
    query: web::Query<EmailSearchQuery>,
) -> Result<impl Responder, ApiError> {
    log::info!("Handling GET /emails/search with query: {:?}", query);
    // Note: This assumes a folder is implicitly selected (e.g., INBOX by default
    // or by a previous `select_folder` call). A real implementation might require
    // the folder name in the path or query params.

    // Build SearchCriteria from query params (example)
    let criteria = if query.subject.is_some() {
        SearchCriteria::Subject(query.subject.clone().unwrap())
    } else if query.from.is_some() {
        SearchCriteria::From(query.from.clone().unwrap())
    } else if query.body.is_some() {
        SearchCriteria::Body(query.body.clone().unwrap())
    } else if query.unseen.is_some() {
        SearchCriteria::Unseen
    } else {
        SearchCriteria::All // Default
    };

    log::debug!("Constructed search criteria: {:?}", criteria);
    let uids = state.imap_client.search_emails(criteria).await?;
    Ok(HttpResponse::Ok().json(json!({ "uids": uids })))
}

// GET /emails/fetch?uids=1,2,3
#[derive(Deserialize, Debug)]
struct EmailFetchQuery { uids: String }

async fn fetch_emails(
    state: web::Data<AppState>,
    query: web::Query<EmailFetchQuery>,
) -> Result<impl Responder, ApiError> {
    log::info!("Handling GET /emails/fetch with uids: {}", query.uids);
    let uids = parse_uids(&query.uids)?;
    if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided in query string".to_string()));
    }
    // Note: Assumes a folder is selected.
    let emails = state.imap_client.fetch_emails(uids).await?;
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

// --- Service Configuration ---

// Renamed function to be more specific
pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
    log::info!("Configuring REST API routes under /api/v1");
    cfg.service(
        web::scope("/api/v1") // Base path for this API version
            .route("/health", web::get().to(health_check))
            .service(
                web::scope("/folders")
                    .route("", web::get().to(list_folders))
                    .route("", web::post().to(create_folder))
                    .route("/{folder_name}", web::delete().to(delete_folder))
                    // Route for rename needs path param and body
                    .route("/{folder_name}", web::put().to(rename_folder))
                     // POST for select action
                    .route("/{folder_name}/select", web::post().to(select_folder))
            )
            .service(
                web::scope("/emails")
                    // Search endpoint (example uses query params)
                    .route("/search", web::get().to(search_emails))
                    // Fetch endpoint (requires UIDs in query)
                    .route("/fetch", web::get().to(fetch_emails))
                    // Move endpoint
                    .route("/move", web::post().to(move_emails))
            )
            // Add other resources/scopes here
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