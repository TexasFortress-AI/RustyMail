use actix_web::{
    error as actix_error, get, http::StatusCode, middleware::Logger, post, web, App,
    Error as ActixError, HttpRequest, HttpResponse, HttpServer, Responder, Result as ActixResult,
    ResponseError,
};
use actix_web_lab::middleware::from_fn as mw_from_fn;
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;
use urlencoding;
use uuid::Uuid;

use crate::{
    api::{
        mcp_sse::SseState, // Use state from mcp_sse module
        // sse::SseManager, // Removed - Belongs to dashboard?
        // ApiContext, // Removed - Not defined/used
    },
    config::{RestConfig, Settings},
    // dashboard::services::DashboardState, // Explicitly commented out per user request
    imap::{
        error::ImapError,
        session::{ // Session trait and factory
            ImapSession, ImapSessionFactory,
        },
        types::{ // Specific IMAP data types
            AppendEmailPayload, FlagOperation, Flags, Folder, Email, MailboxInfo, ModifyFlagsPayload, 
            SearchCriteria, StoreOperation,
        },
        // client::ImapClient, // Only needed if used directly, not just via factory
    },
    mcp::{
        handler::McpHandler,
        types::{JsonRpcResponse, McpPortState}, // Types related to MCP interaction
        // McpTool // Not directly used in REST handlers
    },
    // prelude::*, // Avoid wildcard imports unless strictly necessary
};

use crate::imap::session::StoreOperation; // Import StoreOperation from session
use async_imap::error::Error as AsyncImapError;

use crate::api::mcp::types::{JsonRpcRequest, JsonRpcError};

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("IMAP Error: {0}")]
    Imap(#[from] ImapError),
    #[error("Serialization Error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("URL Decoding Error: {0}")]
    UrlDecoding(String), // urlencoding::decode does not return a std::error::Error compatible error
    #[error("Invalid Request: {0}")]
    InvalidRequest(String),
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    #[error("Actix Web Error: {0}")]
    ActixWeb(#[from] ActixError), // Ensure ActixError is imported correctly
    #[error("Authentication Required")]
    Unauthorized,
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    // Added to handle errors from the AI provider interaction
    #[error("AI Provider Error: {0}")]
    AiProviderError(String),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match *self {
            ApiError::Imap(ref err) => match err {
                ImapError::AuthenticationFailed => StatusCode::UNAUTHORIZED,
                ImapError::ConnectionFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
                ImapError::MailboxNotFound(_) => StatusCode::NOT_FOUND,
                ImapError::MailboxAlreadyExists(_) => StatusCode::CONFLICT,
                ImapError::MessageNotFound(_) => StatusCode::NOT_FOUND,
                ImapError::InvalidUidSet(_) => StatusCode::BAD_REQUEST,
                ImapError::OperationFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ImapError::ParseError(_) => StatusCode::BAD_REQUEST, // Or INTERNAL_SERVER_ERROR
                ImapError::SessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ImapError::TlsError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ImapError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ImapError::ReadOnlyError(_) => StatusCode::FORBIDDEN,
            },
            ApiError::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::UrlDecoding(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ActixWeb(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::AiProviderError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();
        let error_message = self.to_string();
        error!("API Error Response ({}): {}", status_code, error_message);
        HttpResponse::build(status_code)
            .content_type("application/json")
            .json(json!({ "error": error_message }))
    }
}

impl From<AsyncImapError> for ApiError {
    fn from(error: AsyncImapError) -> Self {
        ApiError::ImapError(ImapError::from(error))
    }
}

// Helper function to parse comma-separated UIDs
fn parse_uids(uids_str: &str) -> Result<Vec<u32>, ApiError> {
    uids_str
        .split(',')
        .map(|s| {
            s.trim().parse::<u32>().map_err(|e| ApiError::InvalidRequest(format!("Invalid UID format: {}", e)))
        })
        .collect()
}

// --- Configuration and State ---

// Application state shared across handlers
#[derive(Clone)] // Ensure AppState can be cloned for Actix data sharing
pub struct AppState {
    pub imap_session_factory: ImapSessionFactory,
    pub mcp_handler: Arc<dyn McpHandler>,
    pub sse_state: Arc<TokioMutex<SseState>>,
    pub dashboard_state: crate::dashboard::services::DashboardState,
}

impl AppState {
    pub fn new(
        imap_session_factory: ImapSessionFactory,
        mcp_handler: Arc<dyn McpHandler>,
        sse_state: Arc<TokioMutex<SseState>>,
        dashboard_state: crate::dashboard::services::DashboardState,
    ) -> Self {
        Self {
            imap_session_factory,
            mcp_handler,
            sse_state,
            dashboard_state,
        }
    }

    // Accessor for IMAP Session Factory
    pub fn imap_session_factory(&self) -> &ImapSessionFactory {
        &self.imap_session_factory
    }

    // Accessor for MCP Handler
    pub fn mcp_handler(&self) -> &Arc<dyn McpHandler> {
        &self.mcp_handler
    }

    // Accessor for SSE State
    pub fn sse_state(&self) -> &Arc<TokioMutex<SseState>> {
        &self.sse_state
    }
}

// Define ImapSessionFactory type alias using ImapClient directly
// MOVED to src/imap/session_manager.rs or similar central place
// pub type ImapSessionFactory = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<ImapClient, ImapError>> + Send>> + Send + Sync>;

// Helper function to get a session (Refactored to use factory from AppState)
async fn get_session(context: &web::Data<AppState>, _account: &str) -> Result<Pin<Box<dyn ImapSession>>, ApiError> {
    // TODO: Implement account selection logic if needed, currently ignoring account parameter
    // For now, assume the factory provides the correct session for the configured user.
    info!("Attempting to get IMAP session via factory...");
    let session_factory = Arc::clone(&context.imap_session_factory);
    let client = (session_factory)().await?; // Call the factory closure
    info!("Successfully obtained IMAP client via factory.");
    // Wrap the client in the session abstraction
    // Assuming ImapClient implements ImapSession
    let session: Pin<Box<dyn ImapSession>> = Box::pin(client);
    Ok(session)
}

// --- Route Handlers ---

// GET /api/v1/health
#[get("/health")] // Shortened path, assuming no base path like /api/v1 is added globally
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "OK" }))
}

// --- Email Operations ---

#[derive(Deserialize, Debug)]
struct GetEmailsQuery {
    folder: Option<String>,
    #[serde(default)]
    search: String, // Default to empty string if not provided
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    50 // Default number of emails to fetch
}

#[get("/accounts/{account}/emails")]
async fn get_emails(
    state: web::Data<AppState>,
    path: web::Path<String>, // account
    query: web::Query<GetEmailsQuery>,
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling GET /accounts/{}/emails with query: {:?}", account, query);

    let mut session = get_session(&state, &account).await?; // Pass AppState directly

    let folder_name = query.folder.clone().unwrap_or_else(|| "INBOX".to_string());
    info!("REST: Selecting folder: {}", folder_name);
    session.select_folder(&folder_name).await?;
    
    let criteria = if query.search.is_empty() {
        SearchCriteria::All
    } else {
        // Unbox the criteria
        SearchCriteria::Or(vec![
            Box::new(SearchCriteria::Subject(query.search.clone())),
            Box::new(SearchCriteria::From(query.search.clone())),
            // Box::new(SearchCriteria::Body(query.search.clone())), // Keep commented
        ])
    };
    info!("REST: Searching emails with criteria: {:?}", criteria);

    // Pass criteria by value now
    let uids = session.search_emails(criteria).await?;
    info!("REST: Found {} email UIDs matching criteria.", uids.len());

    let total_emails = uids.len();
    let paginated_uids: Vec<u32> = uids.into_iter().skip(query.offset).take(query.limit).collect();
    info!("REST: Fetching details for {} emails (offset: {}, limit: {}).", paginated_uids.len(), query.offset, query.limit);

    if paginated_uids.is_empty() {
        info!("REST: No emails to fetch after pagination.");
        return Ok(HttpResponse::Ok().json(json!({
            "emails": Vec::<Email>::new(),
            "total": total_emails,
            "offset": query.offset,
            "limit": query.limit,
        })));
    }

    // Correct fetch_emails call: pass Vec<u32> and fetch_body flag (default false for now)
    let emails: Vec<Email> = session.fetch_emails(paginated_uids, false).await?;
    info!("REST: Successfully fetched details for {} emails.", emails.len());

    Ok(HttpResponse::Ok().json(json!({
        "emails": emails,
        "total": total_emails,
        "offset": query.offset,
        "limit": query.limit,
    })))
}

#[derive(Deserialize)]
struct MoveEmailPayload {
    target_folder: String,
}

// POST /accounts/{account}/emails/{uids}/move
#[post("/accounts/{account}/emails/{uids}/move")]
async fn move_emails(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>, // (account, uids)
    payload: web::Json<MoveEmailPayload>,
) -> Result<HttpResponse, ApiError> {
    let (account, uids_str) = path.into_inner();
    info!("REST: Handling POST /accounts/{}/emails/{}/move to folder '{}'", account, uids_str, payload.target_folder);

    let uids = parse_uids(&uids_str)?;
    if uids.is_empty() {
        return Err(ApiError::InvalidRequest("No valid UIDs provided.".to_string()));
    }
    info!("REST: Parsed UIDs: {:?}", uids);

    let mut session = get_session(&state, &account).await?;

    // It's generally safer to specify the source folder if known,
    // but many IMAP MOVE commands work within the currently selected folder context.
    // Assuming the emails are in the currently selected folder (e.g., INBOX if not specified).
    // If emails could be in *any* folder, the `move_emails` implementation needs
    // to handle that (possibly requiring a source_folder parameter).
    // For now, assume move operates on the selected folder context.
    // let current_folder = session.get_selected_folder().await?; // Need a method like this
    // if current_folder.is_none() {
    //    return Err(ApiError::InvalidRequest("No folder selected to move emails from.".to_string()));
    // }

    let target_folder = payload.target_folder.clone();
    info!("REST: Moving {} emails to folder '{}'", uids.len(), target_folder);

    // Call move_emails (plural) as defined in the trait
    session.move_emails(uids, &target_folder).await?;

    info!("REST: Emails moved successfully.");
    Ok(HttpResponse::Ok().json(json!({ "status": "Emails moved successfully" })))
}

#[derive(Deserialize, Debug)]
struct UpdateFlagsPayload {
    operation: String,
    flags: Vec<String>,
}

// Helper to parse string flags (assuming Flags::from_str or similar exists)
fn parse_flags(flag_strings: &[String]) -> Result<Vec<Flags>, ApiError> {
    flag_strings.iter().map(|s| {
        match s.to_lowercase().as_str() {
            // Escape backslashes properly
            "\\seen" => Ok(Flags::seen()), 
            "\\answered" => Ok(Flags::answered()),
            "\\flagged" => Ok(Flags::flagged()),
            "\\deleted" => Ok(Flags::deleted()),
            "\\draft" => Ok(Flags::draft()),
            custom => {
                // Keep custom flag logic
                if custom.starts_with('\\') || custom.starts_with('$') { 
                     Ok(Flags::custom(custom))
                } else {
                     Err(ApiError::InvalidRequest(format!("Invalid flag format: {}", s)))
                }
            }
        }
    }).collect()
}

// POST /accounts/{account}/emails/{uids}/flags
#[post("/accounts/{account}/emails/{uids}/flags")]
async fn update_flags(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>, // (account, uids)
    payload: web::Json<UpdateFlagsPayload>,
) -> Result<HttpResponse, ApiError> {
    let (account, uids_str) = path.into_inner();
    info!("REST: Handling POST /accounts/{}/emails/{}/flags with payload: {:?}", account, uids_str, payload);

    let uids = parse_uids(&uids_str)?;
    if uids.is_empty() {
        return Err(ApiError::InvalidRequest("No valid UIDs provided.".to_string()));
    }
    info!("REST: Parsed UIDs: {:?}", uids);

    let operation = match payload.operation.to_lowercase().as_str() {
        "add" => StoreOperation::Add,
        "remove" => StoreOperation::Remove,
        "set" => StoreOperation::Set,
        _ => return Err(ApiError::InvalidRequest("Invalid flag operation specified. Use 'add', 'remove', or 'set'.".to_string())),
    };
    info!("REST: Flag operation: {:?}", operation);

    // Use the helper function to parse flags
    let flags_to_modify = parse_flags(&payload.flags)?;
    info!("REST: Parsed flags to modify: {:?}", flags_to_modify);

    let mut session = get_session(&state, &account).await?;

    // Construct the payload expected by the trait method
    let modify_payload = ModifyFlagsPayload {
        uids: uids.clone(),
        operation,
        flags: flags_to_modify, // Pass the parsed Vec<Flags>
    };

    info!("REST: Storing flags for {} emails.", uids.len());
    // Call store_flags with the payload struct
    session.store_flags(modify_payload).await?;

    info!("REST: Flags updated successfully.");
    Ok(HttpResponse::Ok().json(json!({ "status": "Flags updated successfully" })))
}

// --- Folder Operations ---

#[derive(Deserialize)]
struct CreateFolderPayload {
    name: String,
}

// POST /accounts/{account}/folders
#[post("/accounts/{account}/folders")]
async fn create_folder_handler( // Renamed from create_folder to avoid conflict with imap session method
    state: web::Data<AppState>,
    path: web::Path<String>, // account
    payload: web::Json<CreateFolderPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling POST /accounts/{}/folders with name '{}'", account, payload.name);

    if payload.name.trim().is_empty() {
        return Err(ApiError::InvalidRequest("Folder name cannot be empty.".to_string()));
    }

    let mut session = get_session(&state, &account).await?;
    let folder_name = payload.name.clone(); // Keep original name for response

    // Consider if folders should always be under INBOX or allow top-level.
    // Prepending INBOX. is common practice.
    let full_folder_path = format!("INBOX.{}", folder_name);
    info!("REST: Attempting to create folder: {}", full_folder_path);

    session.create_folder(&full_folder_path).await?;

    info!("REST: Folder '{}' created successfully.", full_folder_path);
    Ok(HttpResponse::Created().json(json!({
        "status": "Folder created successfully",
        "name": folder_name, // Return the user-provided name
        "fullPath": full_folder_path // Return the actual path created
    })))
}

// GET /accounts/{account}/folders
#[get("/accounts/{account}/folders")]
async fn list_folders_handler( // Renamed from list_folders
    state: web::Data<AppState>,
    path: web::Path<String>, // account
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling GET /accounts/{}/folders", account);

    let session = get_session(&state, &account).await?;

    info!("REST: Listing folders...");
    let folders: Vec<Folder> = session.list_folders().await?;
    info!("REST: Found {} folders.", folders.len());

    Ok(HttpResponse::Ok().json(folders))
}

// GET /accounts/{account}/folders/{folder_name}/status
#[get("/accounts/{account}/folders/{folder_name}/status")]
async fn get_folder_status(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>, // (account, folder_name)
) -> Result<HttpResponse, ApiError> {
    let (account, encoded_folder_name) = path.into_inner();
    let folder_name = urlencoding::decode(&encoded_folder_name)
        .map_err(|_| ApiError::InvalidRequest("Invalid folder name encoding.".to_string()))?
        .into_owned();

    info!("REST: Handling GET /accounts/{}/folders/{}/status", account, folder_name);

    let session = get_session(&state, &account).await?;

    info!("REST: Getting status for folder '{}'", folder_name);
    let mailbox_info: MailboxInfo = session.get_folder_status(&folder_name).await?; // Assuming get_folder_status returns MailboxInfo or similar

    info!("REST: Folder status retrieved: {:?}", mailbox_info);
    Ok(HttpResponse::Ok().json(mailbox_info))
}

// --- Email Appending ---

#[derive(Deserialize)]
struct AppendEmailJsonPayload {
    folder: String,
    // Raw email content, base64 encoded
    raw_email_b64: String,
    flags: Option<Vec<String>>,
    datetime: Option<DateTime<Utc>>,
}

// POST /accounts/{account}/emails/append
#[post("/accounts/{account}/emails/append")]
async fn append_email(
    state: web::Data<AppState>,
    path: web::Path<String>, // account
    payload: web::Json<AppendEmailJsonPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling POST /accounts/{}/emails/append to folder '{}'", account, payload.folder);

    let folder_name = payload.folder.clone();
    if folder_name.trim().is_empty() {
        return Err(ApiError::InvalidRequest("Target folder name cannot be empty.".to_string()));
    }

    let raw_email = general_purpose::STANDARD.decode(&payload.raw_email_b64)
        .map_err(|e| ApiError::InvalidRequest(format!("Invalid base64 encoding for raw_email_b64: {}", e)))?;
    info!("REST: Decoded {} bytes of raw email data.", raw_email.len());

    // Parse flags using the helper function
    let flags_option = payload.flags.as_ref().map(|f| parse_flags(f)).transpose()?;
    info!("REST: Parsed flags for append: {:?}", flags_option);

    let mut session = get_session(&state, &account).await?;

    // Construct the payload struct defined in imap::types
    let append_payload_struct = AppendEmailPayload {
        folder: folder_name.clone(), // Use folder_name
        message: raw_email, // Use decoded raw_email
        flags: flags_option, // Use parsed Option<Vec<Flags>>
        internal_date: payload.datetime, 
    };

    info!("REST: Appending email to folder '{}'...", folder_name);
    // Call append_email (plural) with the payload struct
    session.append_email(append_payload_struct).await?;

    info!("REST: Email appended successfully to folder '{}'.", folder_name);
    Ok(HttpResponse::Created().json(json!({ "status": "Email appended successfully" })))
}

// --- Service Configuration ---

// This function configures the services for the REST API part of the application
pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
     info!("Configuring REST API services...");
    cfg.service(
        web::scope("/api/v1") // Base path for V1 API
            .service(health_check)
            // Account-specific routes
            .service(
                web::scope("/accounts/{account}")
                    .service(get_emails)
                    .service(move_emails)
                    .service(update_flags)
                    .service(create_folder_handler) // Renamed handler
                    .service(list_folders_handler) // Renamed handler
                    .service(get_folder_status)
                    .service(append_email) // Append is under /emails/append now, moved out of /{account} scope? Let's keep it here for now.
                    // Potentially add DELETE /emails/{uids} here too
            )
            // Maybe non-account specific routes if needed later?
            // .service(some_global_resource)
    );
     info!("REST API services configured.");
}

// --- Main Server Function (moved to main.rs or lib.rs) ---
/*
pub async fn start_rest_api_server(
    settings: Settings,
    imap_session_factory: ImapSessionFactory,
    mcp_handler: Arc<dyn McpHandler + Send + Sync>,
    sse_state: Arc<SseState>, // For potential future integration or shared state
    dashboard_state: DashboardState, // Assuming dashboard state is needed
    sse_manager: Arc<SseManager>,    // Assuming dashboard SSE manager is needed
) -> std::io::Result<()> {

    let rest_config = settings.rest.clone().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "REST configuration missing")
    })?;

    if !rest_config.enabled {
        info!("REST API is disabled in the configuration.");
        return Ok(()); // Exit gracefully if disabled
    }

    let listen_addr = format!("{}:{}", rest_config.host, rest_config.port);
    info!("Starting REST API server at http://{}", listen_addr);

    let app_state = AppState::new(
        imap_session_factory.clone(),
        mcp_handler.clone(),
        sse_state.clone(),
        dashboard_state.clone(), // Clone dashboard state for AppState
    );


    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone())) // Share AppState
            .app_data(web::Data::new(settings.clone())) // Share Settings if needed by handlers
            .app_data(web::Data::new(sse_manager.clone())) // Share Dashboard SSE Manager
            .wrap(Logger::default()) // Logging middleware
            // Add authentication middleware if needed here
            // .wrap(AuthMiddleware::new())
            .configure(configure_rest_service) // Mount API routes
            // Mount dashboard API if it's part of the same server
            .configure(|cfg| dashboard::api::init_routes(cfg)) // Assumes dashboard uses same AppState/Data
    })
    .bind(&listen_addr)?
    .run()
    .await
}
*/

// --- Tests (Optional: Keep or move to a separate test file) ---

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    // Add necessary mocks or test setup here

    #[actix_rt::test]
    async fn test_health_check() {
        let app = test::init_service(App::new().service(health_check)).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body, json!({ "status": "OK" }));
    }

    // Add more tests for other endpoints using mock sessions/factories
}
