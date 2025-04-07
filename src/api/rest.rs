use actix_web::{web, App, HttpServer, Responder, HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use std::sync::Arc;
use thiserror::Error; // For defining API-specific errors
use serde::Deserialize; // Added for email payloads
use crate::imap::error::ImapError as InternalImapError;
use crate::config::RestConfig;
use log::{error};
use serde_json::json; // Make sure json macro is imported
use crate::imap::client::ImapClient;
use crate::imap::types::{SearchCriteria}; // Include SearchCriteria
use urlencoding;

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

// Define the application state shared across handlers
#[derive(Clone)] // Ensure AppState is Clone
struct AppState {
    // Uncomment imap_client field
    imap_client: Arc<ImapClient>,
    rest_config: RestConfig, 
}

// --- Route Handlers ---

// GET /api/v1/health
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// GET /folders
async fn list_folders(app_state: web::Data<AppState>) -> impl Responder {
    // Uncomment handler body
    match app_state.imap_client.list_folders().await {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(e) => {
            error!("Failed to list folders: {}", e);
            // Use ApiError or simplify error response
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

// POST /folders { "name": "folder_name" }
#[derive(Deserialize)]
struct FolderCreatePayload { name: String }

async fn create_folder(app_state: web::Data<AppState>, payload: web::Json<FolderCreatePayload>) -> impl Responder {
    // Uncomment handler body
    let name = &payload.name;
    if name.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({ "error": "Folder name cannot be empty" }));
    }
    match app_state.imap_client.create_folder(name).await {
        Ok(_) => HttpResponse::Created().json(json!({ "message": format!("Folder '{}' created", name) })),
        Err(e) => {
            error!("Failed to create folder '{}': {}", name, e);
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

// DELETE /folders/{folder_name}
async fn delete_folder(app_state: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    // Uncomment handler body
    let name = path.into_inner();
    // Basic URL decode (consider more robust library if needed)
    let decoded_name = match urlencoding::decode(&name) {
        Ok(decoded) => decoded.into_owned(),
        Err(e) => {
            error!("Failed to URL decode folder name '{}': {}", name, e);
            return HttpResponse::BadRequest().json(json!({ "error": "Invalid folder name encoding" }));
        }
    };
    match app_state.imap_client.delete_folder(&decoded_name).await {
        Ok(_) => HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' deleted", decoded_name) })),
        Err(e) => {
            error!("Failed to delete folder '{}': {}", decoded_name, e);
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

// PUT /folders/{from_name} { "to_name": "new_name" }
#[derive(Deserialize)]
struct FolderRenamePayload { to_name: String }

async fn rename_folder(app_state: web::Data<AppState>, path: web::Path<String>, payload: web::Json<FolderRenamePayload>) -> impl Responder {
    // Uncomment handler body
    let from_name = path.into_inner();
    let to_name = &payload.to_name;
    // Decode names
    let decoded_from = match urlencoding::decode(&from_name) {
        Ok(decoded) => decoded.into_owned(),
        Err(e) => return HttpResponse::BadRequest().json(json!({ "error": format!("Invalid 'from' name encoding: {}", e) })),
    };
     let decoded_to = match urlencoding::decode(&to_name) {
        Ok(decoded) => decoded.into_owned(),
        Err(e) => return HttpResponse::BadRequest().json(json!({ "error": format!("Invalid 'to' name encoding: {}", e) })),
    };
     if decoded_to.trim().is_empty() {
         return HttpResponse::BadRequest().json(json!({ "error": "New folder name cannot be empty" }));
     }

    match app_state.imap_client.rename_folder(&decoded_from, &decoded_to).await {
        Ok(_) => HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' renamed to '{}'", decoded_from, decoded_to) })),
        Err(e) => {
            error!("Failed to rename folder '{}' to '{}': {}", decoded_from, decoded_to, e);
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

// POST /folders/{folder_name}/select
async fn select_folder(app_state: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    // Uncomment handler body
    let name = path.into_inner();
    let decoded_name = match urlencoding::decode(&name) {
        Ok(decoded) => decoded.into_owned(),
        Err(e) => return HttpResponse::BadRequest().json(json!({ "error": format!("Invalid folder name encoding: {}", e) })),
    };
    match app_state.imap_client.select_folder(&decoded_name).await {
        // TODO: Determine how to serialize async_imap::types::Mailbox if needed
        // Ok(mailbox_info) => HttpResponse::Ok().json(mailbox_info), 
        Ok(_) => HttpResponse::Ok().json(json!({ "message": format!("Folder '{}' selected", decoded_name)})), // Temp response
        Err(e) => {
            error!("Failed to select folder '{}': {}", decoded_name, e);
            HttpResponse::InternalServerError().json(json!({ "error": e.to_string() }))
        }
    }
}

// GET /folders/{name}/emails (Search - simplified)
#[derive(Deserialize, Debug)]
struct EmailSearchQuery { subject: Option<String>, from: Option<String> }
async fn search_emails_handler(
    state: web::Data<AppState>,
    folder_name: web::Path<String>,
    query: web::Query<EmailSearchQuery>,
) -> Result<impl Responder, ApiError> {
    // Uncomment handler body (Simplified example criteria)
    // Select folder first (often required)
    let decoded_folder = urlencoding::decode(&folder_name).map_err(|_| ApiError::BadRequest("Invalid folder encoding".to_string()))?.into_owned();
    state.imap_client.select_folder(&decoded_folder).await?; // Use `?` to propagate ImapError -> ApiError

    // Build criteria (example)
    let criteria = if let Some(subj) = &query.subject {
        SearchCriteria::Subject(subj.clone()) 
    } else if let Some(sender) = &query.from {
         SearchCriteria::From(sender.clone())
    } else {
        SearchCriteria::All
    };

    let uids = state.imap_client.search_emails(criteria).await?;
    Ok(HttpResponse::Ok().json(json!({ "uids": uids })))
}

// GET /emails/fetch?uids=1,2,3 
#[derive(Deserialize, Debug)]
struct EmailFetchQuery { uids: String }
async fn fetch_emails_handler(
    state: web::Data<AppState>,
    query: web::Query<EmailFetchQuery>,
) -> Result<impl Responder, ApiError> {
    // Uncomment handler body
    let uids: Vec<u32> = query.uids.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if uids.is_empty() {
        return Err(ApiError::BadRequest("Invalid or empty UIDs provided".to_string()));
    }
    // Note: Assumes a folder is already selected or fetch works across folders
    let emails = state.imap_client.fetch_emails(uids).await?;
    Ok(HttpResponse::Ok().json(emails))
}

// POST /emails/move { "uids": [1, 2], "destination_folder": "Archive" }
#[derive(Deserialize)]
struct EmailMovePayload { uids: Vec<u32>, destination_folder: String }
async fn move_email_handler(
    state: web::Data<AppState>,
    payload: web::Json<EmailMovePayload>,
) -> Result<impl Responder, ApiError> {
    // Uncomment handler body
    let uids = payload.uids.clone();
    if uids.is_empty() {
        return Err(ApiError::BadRequest("UID list cannot be empty".to_string()));
    }
    let dest = &payload.destination_folder;
     if dest.trim().is_empty() {
        return Err(ApiError::BadRequest("Destination folder cannot be empty".to_string()));
    }
    // Note: Assumes source folder is selected
    state.imap_client.move_email(uids.clone(), dest).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": "Emails moved successfully" })))
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
                    .route("", web::get().to(list_folders))
                    .route("", web::post().to(create_folder))
                    .route("/{name}", web::delete().to(delete_folder))
                    .route("/{name}/rename", web::put().to(rename_folder))
                    .route("/{name}/select", web::post().to(select_folder))
                    // Route to search emails within a specific folder
                    .route("/{name}/emails", web::get().to(search_emails_handler))
            )
            .service(
                 web::scope("/emails")
                    // Route to fetch specific emails by UID (comma-separated)
                    .route("/fetch", web::get().to(fetch_emails_handler)) // Requires uids query param
                    // Route to move emails
                    .route("/move", web::post().to(move_email_handler))
            )
    );
}

// --- Server Initialization ---

pub async fn run_server(
    config: RestConfig,
    imap_client: Arc<ImapClient>, // Use the parameter
  ) -> std::io::Result<()> {
    
    let app_state = web::Data::new(AppState {
        // Uncomment imap_client field assignment
        imap_client: imap_client.clone(),
        rest_config: config.clone(),
    });

    log::info!("Starting Actix server at http://{}:{}", config.host, config.port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone()) // Add shared state
            .wrap(Logger::default()) // Enable basic logging
            .service(
                web::scope("/api/v1") // Group routes under /api/v1
                    // Restore actual handlers
                    .route("/folders", web::get().to(list_folders))
                    .route("/folders", web::post().to(create_folder))
                    .route("/folders/{name}", web::delete().to(delete_folder))
                    .route("/folders/{name}/rename", web::put().to(rename_folder))
                    .route("/folders/{name}/select", web::post().to(select_folder))
                    .route("/folders/{name}/emails", web::get().to(search_emails_handler))
                    .route("/emails/fetch", web::get().to(fetch_emails_handler))
                    .route("/emails/move", web::post().to(move_email_handler))
            )
    })
    .bind((config.host.as_str(), config.port))?
    .run()
    .await
} 