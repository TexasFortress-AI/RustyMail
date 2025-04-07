use crate::prelude::*; // MOVE TO TOP
use actix_web::{web, App, HttpServer, Responder, HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use std::sync::Arc;
use thiserror::Error; // For defining API-specific errors
use serde::Deserialize; // Added for email payloads
use crate::imap::error::ImapError as InternalImapError;
use crate::config::RestConfig;

// --- API Specific Error Handling ---

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    #[error("IMAP Error")]
    ImapError(#[from] InternalImapError),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ImapError(ref imap_err) => {
                match imap_err {
                    InternalImapError::Auth(_) => StatusCode::UNAUTHORIZED,
                    InternalImapError::Mailbox(_) => StatusCode::NOT_FOUND,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                }
            },
        }
    }

    fn error_response(&self) -> HttpResponse {
        let message = match self {
            ApiError::InternalError(_) => "Internal Server Error".to_string(),
            _ => self.to_string()
        };
        HttpResponse::build(self.status_code())
            .json(serde_json::json!({ "error": message }))
    }
}

// Helper to convert string UIDs to Vec<u32>
fn parse_uids(uids_str: &str) -> Result<Vec<u32>, ApiError> {
    uids_str.split(',')
        .map(|s| s.trim().parse::<u32>()
             .map_err(|_| ApiError::BadRequest(format!("Invalid UID format: {}", s))))
        .collect()
}

// --- Configuration and State ---

// Placeholder for application state, including the IMAP client
// We'll likely inject the ImapClient or ImapSession trait object here
pub struct AppState {
    imap_client: Arc<ImapClient>, // Or Box<dyn ImapSession>
}

// --- Route Handlers ---

// GET /api/v1/health
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// GET /api/v1/folders
async fn list_folders_handler(state: web::Data<AppState>) -> Result<impl Responder, ApiError> {
    let folders = state.imap_client.list_folders().await?;
    Ok(HttpResponse::Ok().json(folders))
}

// POST /api/v1/folders
#[derive(Deserialize)]
struct FolderCreatePayload {
    name: String,
}
async fn create_folder_handler(
    state: web::Data<AppState>,
    payload: web::Json<FolderCreatePayload>,
) -> Result<impl Responder, ApiError> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::BadRequest("Folder name cannot be empty".to_string()));
    }
    state.imap_client.create_folder(&payload.name).await?;
    // Consider returning the created folder representation or just 201
    Ok(HttpResponse::Created().json(serde_json::json!({ "message": "Folder created", "name": payload.name })))
}

// DELETE /api/v1/folders/{name}
async fn delete_folder_handler(
    state: web::Data<AppState>,
    folder_name: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    state.imap_client.delete_folder(&folder_name).await?;
    Ok(HttpResponse::NoContent().finish())
}

// PUT /api/v1/folders/{name}/rename
#[derive(Deserialize)]
struct FolderRenamePayload {
    new_name: String,
}
async fn rename_folder_handler(
    state: web::Data<AppState>,
    folder_name: web::Path<String>,
    payload: web::Json<FolderRenamePayload>,
) -> Result<impl Responder, ApiError> {
     if payload.new_name.trim().is_empty() {
        return Err(ApiError::BadRequest("New folder name cannot be empty".to_string()));
    }
    state.imap_client.rename_folder(&folder_name, &payload.new_name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Folder renamed", "old_name": folder_name.into_inner(), "new_name": payload.new_name })))
}

// === Email Handlers ===

// GET /api/v1/folders/{name}/emails?criteria=...&value=...
// Example: /emails?criteria=Subject&value=Test
// Example: /emails?criteria=All
#[derive(Deserialize)]
struct EmailSearchQuery {
    criteria: Option<String>, // e.g., Subject, From, To, Body, Since, Uid, All
    value: Option<String>, 
}
async fn search_emails_handler(
    state: web::Data<AppState>,
    folder_name: web::Path<String>,
    query: web::Query<EmailSearchQuery>,
) -> Result<impl Responder, ApiError> {
    // Select the folder first (implicitly required for search in IMAP)
    state.imap_client.select_folder(&folder_name).await?;
    
    let search_criteria = match query.criteria.as_deref() {
        Some("Subject") => SearchCriteria::Subject(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for Subject criteria".into()))?),
        Some("From") => SearchCriteria::From(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for From criteria".into()))?),
        Some("To") => SearchCriteria::To(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for To criteria".into()))?),
        Some("Body") => SearchCriteria::Body(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for Body criteria".into()))?),
        Some("Since") => SearchCriteria::Since(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for Since criteria".into()))?),
        Some("Uid") => SearchCriteria::Uid(query.value.clone().ok_or_else(|| ApiError::BadRequest("Missing value for Uid criteria".into()))?),
        Some("All") | None => SearchCriteria::All,
        Some(other) => return Err(ApiError::BadRequest(format!("Unsupported search criteria: {}", other)))
    };

    let uids = state.imap_client.search_emails(search_criteria).await?;
    // Optionally fetch summaries here instead of just UIDs
    Ok(HttpResponse::Ok().json(serde_json::json!({ "uids": uids })))
}

// GET /api/v1/emails?uids=1,2,3
// Fetches full email details for comma-separated UIDs
// Note: Assumes a folder has been implicitly selected by a previous operation 
// or requires selecting a folder if the session doesn't maintain state.
// For simplicity, we fetch directly without explicit select here, 
// assuming the underlying session handles it or is stateful across requests (which might not be ideal).
#[derive(Deserialize)]
struct EmailFetchQuery {
    uids: String, 
}
async fn fetch_emails_handler(
    state: web::Data<AppState>,
    query: web::Query<EmailFetchQuery>,
) -> Result<impl Responder, ApiError> {
    let uids = parse_uids(&query.uids)?;
    if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided".to_string()));
    }
    // Potentially requires selecting a folder first depending on session state management.
    // Add: state.imap_client.select_folder("INBOX").await?; // Example: select INBOX first
    let emails = state.imap_client.fetch_emails(uids).await?;
    Ok(HttpResponse::Ok().json(emails))
}

// POST /api/v1/emails/move
#[derive(Deserialize)]
struct EmailMovePayload {
    uids: String, // Comma-separated UIDs
    destination_folder: String,
}
async fn move_email_handler(
    state: web::Data<AppState>,
    payload: web::Json<EmailMovePayload>,
) -> Result<impl Responder, ApiError> {
    let uids = parse_uids(&payload.uids)?;
     if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided".to_string()));
    }
     if payload.destination_folder.trim().is_empty() {
        return Err(ApiError::BadRequest("Destination folder cannot be empty".to_string()));
    }
    // Again, potentially requires selecting the source folder first.
    // Add: state.imap_client.select_folder("SOURCE_FOLDER_NAME").await?; 
    state.imap_client.move_email(uids.clone(), &payload.destination_folder).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ 
        "message": "Emails moved successfully", 
        "uids": uids, 
        "destination": payload.destination_folder 
    })))
}

// --- Routes Configuration ---

// Configuration function for Actix-Web routes
// Make this public for tests
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/health", web::get().to(health_check))
            .service(
                web::scope("/folders")
                    .route("", web::get().to(list_folders_handler))
                    .route("", web::post().to(create_folder_handler))
                    .route("/{name}", web::delete().to(delete_folder_handler))
                    .route("/{name}/rename", web::put().to(rename_folder_handler))
                    // Route to search emails within a specific folder
                    .route("/{name}/emails", web::get().to(search_emails_handler))
            )
            .service(
                 web::scope("/emails")
                    // Route to fetch specific emails by UID (comma-separated)
                    .route("", web::get().to(fetch_emails_handler)) // Requires uids query param
                    // Route to move emails
                    .route("/move", web::post().to(move_email_handler))
            )
    );
}

// --- Server Initialization ---

pub async fn run_server(
    config: RestConfig,
    imap_client: Arc<ImapClient>,
) -> std::io::Result<()> {
    
    let app_state = web::Data::new(AppState {
        imap_client: imap_client.clone(),
    });

    println!("Starting REST server at http://{}:{}", config.host, config.port);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(app_state.clone())
            // We pass the concrete type here for now
            .configure(configure_routes)
    })
    .bind((config.host.as_str(), config.port))?
    .run()
    .await
} 