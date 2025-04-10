use actix_web::{
    error as actix_error,
    http::StatusCode,
    web::{self, Data, Path},
    Error as ActixError,
    HttpRequest, HttpResponse,
    Responder,
    Result as ActixResult,
    ResponseError,
    get, post, put, delete, // Add common HTTP method macros
};
use actix_web_lab::middleware::from_fn as mw_from_fn;
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use log::{error, info, debug, warn}; // Added debug, warn
use serde::{Serialize, Deserialize}; // Added Deserialize
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;
use urlencoding;
use uuid::Uuid;

use crate::{
    api::rest::AppState,
    imap::{
        error::ImapError,
        session::{
            ImapSession, ImapSessionFactory,
        },
        types::{
            AppendEmailPayload, FlagOperation, Flags, Folder, Email, MailboxInfo, ModifyFlagsPayload, 
            SearchCriteria, StoreOperation,
        },
    },
    mcp::{
        handler::McpHandler,
        types::{JsonRpcResponse, McpPortState},
    },
};

use async_imap::error::Error as AsyncImapError;

use crate::api::mcp::types::{JsonRpcRequest, JsonRpcError};

/// Represents errors that can occur within the REST API layer.
///
/// This enum centralizes error handling for the API, mapping various
/// internal errors (IMAP, serialization, validation, etc.) into appropriate
/// HTTP responses with relevant status codes.
#[derive(Error, Debug)]
pub enum ApiError {
    /// An error occurred during an IMAP operation.
    #[error("IMAP Error: {0}")]
    Imap(#[from] ImapError),
    
    /// An error occurred during JSON serialization or deserialization.
    #[error("Serialization Error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// An error occurred while decoding a URL-encoded string (e.g., folder name).
    #[error("URL Decoding Error: {0}")]
    UrlDecoding(String),
    
    /// The request was malformed or contained invalid parameters.
    #[error("Invalid Request: {0}")]
    InvalidRequest(String),
    
    /// An unexpected internal error occurred on the server.
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    
    /// An error originated from the Actix-web framework itself.
    #[error("Actix Web Error: {0}")]
    ActixWeb(#[from] ActixError),
    
    /// Authentication is required, but was not provided or was invalid.
    #[error("Authentication Required")]
    Unauthorized,
    
    /// The requested resource (e.g., folder, email) was not found.
    #[error("Not Found: {0}")]
    NotFound(String),
    
    /// The request conflicts with the current state of the resource (e.g., creating an existing folder).
    #[error("Conflict: {0}")]
    Conflict(String),
    
    /// An error occurred while interacting with an external AI provider.
    #[error("AI Provider Error: {0}")]
    AiProviderError(String),
}

impl ResponseError for ApiError {
    /// Determines the appropriate HTTP status code for the API error.
    fn status_code(&self) -> StatusCode {
        match *self {
            // Map specific IMAP errors to HTTP statuses
            ApiError::Imap(ref err) => match err {
                // Use the actual ImapError variants from src/imap/error.rs
                ImapError::Auth(_) | ImapError::AuthenticationError(_) => StatusCode::UNAUTHORIZED,
                ImapError::Connection(_) | ImapError::ConnectionError(_) | ImapError::Tls(_) => StatusCode::SERVICE_UNAVAILABLE,
                ImapError::FolderNotFound(_) | ImapError::MailboxNotFound(_) => StatusCode::NOT_FOUND,
                ImapError::FolderExists(_) | ImapError::MailboxAlreadyExists(_) => StatusCode::CONFLICT,
                ImapError::EmailNotFound(_) | ImapError::MessageNotFound(_) | ImapError::EnvelopeNotFound => StatusCode::NOT_FOUND,
                ImapError::Parse(_) | ImapError::ParseError(_) | ImapError::InvalidCriteria(_) | ImapError::Encoding(_) => StatusCode::BAD_REQUEST,
                ImapError::Command(_) | ImapError::Operation(_) | ImapError::OperationFailed(_) | ImapError::Append(_) | ImapError::Fetch(_) | ImapError::Mailbox(_) | ImapError::FolderNotSelected | ImapError::RequiresFolderSelection(_) => StatusCode::INTERNAL_SERVER_ERROR, // Or BAD_REQUEST depending on context
                ImapError::Io(_) | ImapError::IoError(_) | ImapError::SessionError(_) | ImapError::Config(_) | ImapError::Internal(_) | ImapError::BadResponse(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            // Map other API errors
            ApiError::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::UrlDecoding(_) => StatusCode::BAD_REQUEST,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ActixWeb(_) => StatusCode::INTERNAL_SERVER_ERROR, // Or inspect ActixError further
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::AiProviderError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Creates the HTTP response for the API error.
    ///
    /// Logs the error and returns a JSON response containing the error message.
    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();
        let error_message = self.to_string();
        // Log the detailed error for server-side debugging
        match self {
            ApiError::Imap(e) => error!("API Error Response ({}) - IMAP Error: {:?}", status_code, e),
            ApiError::Serialization(e) => error!("API Error Response ({}) - Serialization Error: {:?}", status_code, e),
            _ => error!("API Error Response ({}): {}", status_code, error_message),
        }
        // Return a generic error message to the client for internal errors
        let client_message = match status_code {
            StatusCode::INTERNAL_SERVER_ERROR | StatusCode::SERVICE_UNAVAILABLE => "An internal server error occurred".to_string(),
            _ => error_message,
        };
        HttpResponse::build(status_code)
            .content_type("application/json")
            .json(json!({ "error": client_message }))
    }
}

impl From<AsyncImapError> for ApiError {
    fn from(error: AsyncImapError) -> Self {
        ApiError::Imap(ImapError::from(error))
    }
}

/// Parses a comma-separated string of UIDs into a `Vec<u32>`.
///
/// Returns `ApiError::InvalidRequest` if any part fails to parse as a u32.
fn parse_uids(uids_str: &str) -> Result<Vec<u32>, ApiError> {
    if uids_str.is_empty() {
        return Ok(Vec::new()); // Handle empty string gracefully
    }
    uids_str
        .split(',')
        .map(|s| {
            s.trim().parse::<u32>().map_err(|e| {
                error!("Failed to parse UID '{}': {}", s.trim(), e);
                ApiError::InvalidRequest(format!("Invalid UID format: '{}'", s.trim()))
            })
        })
        .collect()
}

// --- Configuration and State ---

/// Holds shared application state accessible by Actix-web handlers.
///
/// Includes factories and state managers for various services like IMAP, MCP, SSE,
/// and potentially dashboard features.
#[derive(Clone)] // Ensure AppState can be cloned for Actix data sharing
pub struct AppState {
    /// Factory function/trait object for creating IMAP sessions.
    pub imap_session_factory: ImapSessionFactory,
    /// Shared MCP request handler.
    pub mcp_handler: Arc<dyn McpHandler>,
    /// Shared state manager for Server-Sent Events.
    pub sse_state: Arc<TokioMutex<SseState>>, // Using SseState from api::sse
    /// State manager for dashboard-related services.
    pub dashboard_state: crate::dashboard::services::DashboardState,
}

impl AppState {
    /// Creates a new `AppState` instance.
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

/// **GET /health**
///
/// Simple health check endpoint.
///
/// # Responses
/// - `200 OK`: Returns `{"status": "OK"}`.
#[get("/health")]
async fn health_check() -> impl Responder {
    debug!("Handling GET /health request");
    HttpResponse::Ok().json(json!({ "status": "OK" }))
}

// --- Email Operations ---

/// Query parameters for the `GET /accounts/{account}/emails` endpoint.
#[derive(Deserialize, Debug)]
struct GetEmailsQuery {
    /// The target folder name. Defaults to "INBOX" if not specified.
    folder: Option<String>,
    /// Optional search string. If provided, searches Subject and From fields.
    /// Defaults to an empty string (effectively `SearchCriteria::All`).
    #[serde(default)]
    search: String,
    /// Maximum number of email summaries to return. Defaults to 50.
    #[serde(default = "default_limit")]
    limit: usize,
    /// Number of emails to skip for pagination. Defaults to 0.
    #[serde(default)]
    offset: usize,
}

/// Default limit for email fetching if not specified in the query.
fn default_limit() -> usize {
    50
}

/// **GET /accounts/{account}/emails**
///
/// Retrieves a list of emails for a given account, with options for filtering and pagination.
///
/// # Path Parameters
/// - `account`: Identifier for the target email account (currently ignored, uses configured default).
///
/// # Query Parameters
/// - `folder` (optional): The folder to list emails from. Defaults to "INBOX".
/// - `search` (optional): A string to search for in email Subject and From fields.
/// - `limit` (optional): Maximum number of emails to return. Defaults to 50.
/// - `offset` (optional): Number of emails to skip (for pagination). Defaults to 0.
///
/// # Responses
/// - `200 OK`: Returns a JSON object containing `emails` (an array of `Email` summaries)
///             and `total_emails` (the total count matching the criteria before pagination).
/// - `400 Bad Request`: If query parameters are invalid (`ApiError::InvalidRequest`).
/// - `404 Not Found`: If the specified folder doesn't exist (`ApiError::Imap(ImapError::MailboxNotFound)`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
/// - `503 Service Unavailable`: If the IMAP connection fails (`ApiError::Imap(ImapError::ConnectionFailed)`).
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

/// Request body for moving emails.
#[derive(Deserialize, Debug)] // Added Debug
struct MoveEmailPayload {
    /// The name of the destination folder.
    target_folder: String,
}

/// **POST /accounts/{account}/emails/{uids}/move**
///
/// Moves one or more emails (identified by UIDs) to a specified target folder.
/// Assumes emails are being moved from the currently selected folder context.
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
/// - `uids`: Comma-separated list of email UIDs to move.
///
/// # Request Body (`MoveEmailPayload`)
/// - `target_folder`: The name of the destination folder.
///
/// # Responses
/// - `200 OK`: Returns `{"status": "Emails moved successfully"}`.
/// - `400 Bad Request`: If UIDs are invalid or `target_folder` is missing (`ApiError::InvalidRequest`).
/// - `404 Not Found`: If the target folder or specified emails don't exist (`ApiError::Imap`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
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
    debug!("REST: Parsed UIDs for move: {:?}", uids);

    let mut session = get_session(&state, &account).await?;

    // TODO: Confirm if source folder needs to be explicitly selected or handled.
    // Current ImapSession::move_emails likely operates on the selected folder context.
    // Consider adding source_folder parameter if needed.
    warn!("REST: Assuming move operation is relative to the currently selected folder.");

    let target_folder = payload.target_folder.clone();
    if target_folder.trim().is_empty() {
        return Err(ApiError::InvalidRequest("Target folder name cannot be empty.".to_string()));
    }
    info!("REST: Moving {} emails with UIDs {:?} to folder '{}'", uids.len(), uids, target_folder);

    // Call move_emails (plural) as defined in the trait
    session.move_emails(uids, &target_folder).await?;

    info!("REST: Emails moved successfully.");
    Ok(HttpResponse::Ok().json(json!({ "status": "Emails moved successfully" })))
}

/// Request body for updating email flags.
#[derive(Deserialize, Debug)]
struct UpdateFlagsPayload {
    /// The operation to perform: "add", "remove", or "set".
    operation: String,
    /// A list of flags (strings) to apply (e.g., "\\Seen", "\\Flagged", "$MyCustomFlag").
    flags: Vec<String>,
}

/// Parses flag strings into `imap::types::Flags` enum variants.
/// Supports standard flags like `\Seen`, `\Answered`, etc., and custom flags.
fn parse_flags(flag_strings: &[String]) -> Result<Vec<Flags>, ApiError> {
    debug!("Parsing flag strings: {:?}", flag_strings);
    flag_strings.iter().map(|s| {
        // Use the FromStr implementation for Flags
        s.parse::<Flags>().map_err(|e| {
            error!("Failed to parse flag '{}': {}", s, e);
            ApiError::InvalidRequest(format!("Invalid flag format: '{}'. Error: {}", s, e))
        })
        /* Old manual matching logic:
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
        */
    }).collect()
}

/// **POST /accounts/{account}/emails/{uids}/flags**
///
/// Modifies flags for one or more emails (identified by UIDs).
/// Allows adding, removing, or setting flags.
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
/// - `uids`: Comma-separated list of email UIDs to modify.
///
/// # Request Body (`UpdateFlagsPayload`)
/// - `operation`: "add", "remove", or "set".
/// - `flags`: Array of flag strings (e.g., `["\\Seen", "$Important"]`).
///
/// # Responses
/// - `200 OK`: Returns `{"status": "Flags updated successfully"}`.
/// - `400 Bad Request`: If UIDs, operation, or flags are invalid (`ApiError::InvalidRequest`).
/// - `404 Not Found`: If specified emails don't exist (`ApiError::Imap`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
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
    debug!("REST: Parsed UIDs for flag update: {:?}", uids);

    // Parse the operation string into the StoreOperation enum
    let operation = match payload.operation.to_lowercase().as_str() {
        "add" => StoreOperation::Add,
        "remove" => StoreOperation::Remove,
        "set" => StoreOperation::Set,
        op => {
             error!("Invalid flag operation received: {}", op);
             return Err(ApiError::InvalidRequest("Invalid flag operation specified. Use 'add', 'remove', or 'set'.".to_string()))
        },
    };
    debug!("REST: Parsed flag operation: {:?}", operation);

    // Use the helper function to parse flag strings into Vec<Flags>
    let flags_to_modify = parse_flags(&payload.flags)?;
    if flags_to_modify.is_empty() {
         return Err(ApiError::InvalidRequest("No flags provided to modify.".to_string()));
    }
    debug!("REST: Parsed flags to modify: {:?}", flags_to_modify);

    let mut session = get_session(&state, &account).await?;

    // Construct the payload expected by the trait method
    let modify_payload = ModifyFlagsPayload {
        uids: uids.clone(),
        operation,
        flags: flags_to_modify,
    };

    info!("REST: Storing flags ({:?}) for {} emails with UIDs {:?}.", modify_payload.operation, uids.len(), uids);
    // Call store_flags with the payload struct
    session.store_flags(modify_payload).await?;

    info!("REST: Flags updated successfully.");
    Ok(HttpResponse::Ok().json(json!({ "status": "Flags updated successfully" })))
}

// --- Folder Operations ---

/// Request body for creating a new folder.
#[derive(Deserialize, Debug)] // Added Debug
struct CreateFolderPayload {
    /// The name of the folder to create (relative to INBOX or root, depending on server).
    name: String,
}

/// **POST /accounts/{account}/folders**
///
/// Creates a new IMAP folder (mailbox).
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
///
/// # Request Body (`CreateFolderPayload`)
/// - `name`: The desired name for the new folder.
///
/// # Responses
/// - `201 Created`: Returns status and the created folder name/path.
/// - `400 Bad Request`: If the name is empty (`ApiError::InvalidRequest`).
/// - `409 Conflict`: If a folder with that name already exists (`ApiError::Imap(ImapError::MailboxAlreadyExists)`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
#[post("/accounts/{account}/folders")]
async fn create_folder_handler( // Renamed from create_folder to avoid conflict with imap session method
    state: web::Data<AppState>,
    path: web::Path<String>, // account
    payload: web::Json<CreateFolderPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling POST /accounts/{}/folders with name '{}'", account, payload.name);

    let folder_name = payload.name.trim();
    if folder_name.is_empty() {
        return Err(ApiError::InvalidRequest("Folder name cannot be empty.".to_string()));
    }

    let mut session = get_session(&state, &account).await?;

    // Note: Folder creation logic might depend on server delimiter and hierarchy preferences.
    // Creating directly under root vs. under INBOX might vary.
    // Let the underlying `create_folder` implementation handle delimiter logic.
    info!("REST: Attempting to create folder: {}", folder_name);
    session.create_folder(folder_name).await?;

    info!("REST: Folder '{}' created successfully.", folder_name);
    // Return 201 Created status
    Ok(HttpResponse::Created().json(json!({ 
        "status": "Folder created successfully",
        "name": folder_name 
    })))
}

/// **GET /accounts/{account}/folders**
///
/// Lists all available folders (mailboxes) for the account.
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
///
/// # Responses
/// - `200 OK`: Returns a JSON array of `Folder` objects.
/// - `500 Internal Server Error`: For IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
#[get("/accounts/{account}/folders")]
async fn list_folders_handler( // Renamed from list_folders
    state: web::Data<AppState>,
    path: web::Path<String>, // account
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling GET /accounts/{}/folders", account);

    // Get session requires mutable borrow if list_folders needs mutable self
    // Assuming list_folders takes &self or session is cloned/recreated.
    let session = get_session(&state, &account).await?;

    info!("REST: Listing folders...");
    let folders: Vec<Folder> = session.list_folders().await?;
    info!("REST: Found {} folders.", folders.len());

    Ok(HttpResponse::Ok().json(folders))
}

/// **GET /accounts/{account}/folders/{folder_name}/status**
///
/// Retrieves status information (message count, unseen count, etc.) for a specific folder.
/// The `folder_name` in the path must be URL-encoded.
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
/// - `folder_name`: URL-encoded name of the target folder.
///
/// # Responses
/// - `200 OK`: Returns a JSON object representing `MailboxInfo`.
/// - `400 Bad Request`: If `folder_name` is not valid URL-encoded (`ApiError::InvalidRequest`).
/// - `404 Not Found`: If the folder doesn't exist (`ApiError::Imap(ImapError::MailboxNotFound)`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
#[get("/accounts/{account}/folders/{folder_name}/status")]
async fn get_folder_status(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>, // (account, folder_name)
) -> Result<HttpResponse, ApiError> {
    let (account, encoded_folder_name) = path.into_inner();
    // Decode the folder name from the URL path
    let folder_name = urlencoding::decode(&encoded_folder_name)
        .map_err(|e| {
            error!("Failed to URL-decode folder name '{}': {}", encoded_folder_name, e);
            ApiError::InvalidRequest(format!("Invalid folder name encoding: {}", encoded_folder_name))
        })?
        .into_owned();

    info!("REST: Handling GET /accounts/{}/folders/'{}'/status", account, folder_name);

    let session = get_session(&state, &account).await?;

    info!("REST: Getting status for folder '{}'", folder_name);
    // Assuming get_folder_status exists on ImapSession and returns MailboxInfo
    let mailbox_info: MailboxInfo = session.get_folder_status(&folder_name).await?;

    debug!("REST: Folder status retrieved: {:?}", mailbox_info);
    Ok(HttpResponse::Ok().json(mailbox_info))
}

// --- Email Appending ---

/// Request body for appending a new email.
#[derive(Deserialize, Debug)] // Added Debug
struct AppendEmailJsonPayload {
    /// The target folder name.
    folder: String,
    /// Raw email content (RFC 822 format), base64 encoded.
    raw_email_b64: String,
    /// Optional list of flags to set on the appended email (e.g., `["\\Seen"]`).
    flags: Option<Vec<String>>,
    /// Optional internal date/time for the email.
    datetime: Option<DateTime<Utc>>,
}

/// **POST /accounts/{account}/emails/append**
///
/// Appends a new email message to the specified folder.
///
/// # Path Parameters
/// - `account`: Identifier for the email account.
///
/// # Request Body (`AppendEmailJsonPayload`)
/// - `folder`: Name of the folder to append the email to.
/// - `raw_email_b64`: Base64 encoded string of the full raw email message (RFC 822 format).
/// - `flags` (optional): Array of flag strings to set initially.
/// - `datetime` (optional): Internal date/time to associate with the message (ISO 8601 format).
///
/// # Responses
/// - `201 Created`: Returns `{"status": "Email appended successfully"}`.
/// - `400 Bad Request`: If folder name is empty, base64 decoding fails, or flags are invalid (`ApiError::InvalidRequest`).
/// - `404 Not Found`: If the target folder doesn't exist (`ApiError::Imap`).
/// - `500 Internal Server Error`: For other IMAP or internal errors (`ApiError::Imap`, `ApiError::InternalError`).
#[post("/accounts/{account}/emails/append")]
async fn append_email(
    state: web::Data<AppState>,
    path: web::Path<String>, // account
    payload: web::Json<AppendEmailJsonPayload>,
) -> Result<HttpResponse, ApiError> {
    let account = path.into_inner();
    info!("REST: Handling POST /accounts/{}/emails/append to folder '{}'", account, payload.folder);

    let folder_name = payload.folder.trim();
    if folder_name.is_empty() {
        return Err(ApiError::InvalidRequest("Target folder name cannot be empty.".to_string()));
    }

    // Decode the base64 email content
    let raw_email = general_purpose::STANDARD.decode(&payload.raw_email_b64)
        .map_err(|e| {
            error!("Failed to base64 decode raw email: {}", e);
             ApiError::InvalidRequest(format!("Invalid base64 encoding for raw_email_b64: {}", e))
        })?;
    debug!("REST: Decoded {} bytes of raw email data for append.", raw_email.len());

    // Parse flags using the helper function, if provided
    let flags_option = payload.flags.as_ref()
        .map(|f_vec| parse_flags(f_vec))
        .transpose()?; // Convert Option<Result<T, E>> to Result<Option<T>, E>
    debug!("REST: Parsed flags for append: {:?}", flags_option);

    let mut session = get_session(&state, &account).await?;

    // Construct the payload struct expected by the underlying ImapSession method
    let append_payload_struct = AppendEmailPayload {
        folder: folder_name.to_string(),
        message: raw_email,
        flags: flags_option,
        internal_date: payload.datetime, 
    };

    info!("REST: Appending email to folder '{}' with flags {:?} and date {:?}...", 
        append_payload_struct.folder, append_payload_struct.flags, append_payload_struct.internal_date);
    
    // Call the session method to perform the append operation
    session.append_email(append_payload_struct).await?;

    info!("REST: Email appended successfully to folder '{}'.", folder_name);
    Ok(HttpResponse::Created().json(json!({ "status": "Email appended successfully" })))
}

// --- Service Configuration ---

/// Configures the Actix-web service for the REST API endpoints.
///
/// This function sets up the routing hierarchy under the `/api/v1` base path
/// and registers all the API handler functions defined in this module.
/// It uses scopes (`web::scope`) to group related endpoints, such as those
/// operating under `/accounts/{account}/`.
///
/// # Arguments
/// * `cfg` - A mutable reference to Actix-web's `ServiceConfig`.
pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
     info!("Configuring REST API services under /api/v1...");
    cfg.service(
        web::scope("/api/v1") // Base path for V1 API
            // --- General Routes --- 
            .service(health_check)
            
            // --- Account-Specific Routes --- 
            .service(
                web::scope("/accounts/{account}") // Scope for account-related operations
                    // -- Email Operations --
                    .service(get_emails)           // GET /emails
                    .service(move_emails)          // POST /emails/{uids}/move
                    .service(update_flags)         // POST /emails/{uids}/flags
                    .service(append_email)         // POST /emails/append 
                                                   // Note: append is outside /emails/{uids} pattern
                    // TODO: Add GET /emails/{uid} for single email fetch?
                    // TODO: Add DELETE /emails/{uids} for deleting emails?
                    
                    // -- Folder Operations --
                    .service(list_folders_handler) // GET /folders
                    .service(create_folder_handler) // POST /folders
                    .service(get_folder_status)    // GET /folders/{folder_name}/status
                    // TODO: Add DELETE /folders/{folder_name} ?
                    // TODO: Add PUT /folders/{old_name}/rename ?
            )
            // --- Add other top-level resources outside /accounts if needed ---
            // .service(some_global_resource)
    );
     info!("REST API services configured.");
}

// --- Main Server Function (moved to main.rs or lib.rs) ---
// The `start_rest_api_server` function previously here is assumed to be 
// part of the application's main binary setup (e.g., in src/main.rs or src/lib.rs),
// responsible for initializing AppState, binding the server, and running it.
/*
pub async fn start_rest_api_server(...)
*/

// --- Tests --- 

/// Contains basic unit/integration tests for the REST API handlers.
/// Uses Actix-web test utilities.
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use crate::imap::error::ImapError; // Import ImapError for potential mocking
    use crate::imap::types::{Email, Folder}; // Import necessary types for mocking/assertions
    use crate::imap::session_manager::{ImapSession, ImapSessionFactory}; // For mocking
    use crate::mcp::McpHandler; // For mocking
    use crate::api::sse::SseState; // For mock AppState
    use crate::dashboard::services::DashboardState; // For mock AppState
    use std::pin::Pin;
    use std::future::Future;
    use mockall::{automock, predicate::*};
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;

    // --- Mock ImapSession --- 
    #[automock]
    pub trait MockImapSession: ImapSession { }

    // --- Mock McpHandler --- 
    #[automock]
    #[async_trait::async_trait]
    pub trait MockMcpHandler: McpHandler { }

    // Helper to create mock AppState
    fn create_mock_app_state() -> AppState {
        // Mock Factory: Returns a mock session
        let factory: ImapSessionFactory = Arc::new(|| {
            let mut mock_session = Box::pin(MockMockImapSession::new()); // Use generated mock
            // Setup default expectations for the session if needed
            mock_session.expect_select_folder().returning(|_| Ok(()));
            mock_session.expect_search_emails().returning(|_| Ok(vec![1, 2, 3]));
            mock_session.expect_fetch_emails().returning(|_, _| Ok(vec![/* mock emails */]));
            // ... add other expectations ...
            let res: Result<Pin<Box<dyn ImapSession>>, ImapError> = Ok(mock_session as Pin<Box<dyn ImapSession>>);
            Box::pin(async move { res }) as Pin<Box<dyn Future<Output = _> + Send>>
        });

        let mock_mcp_handler = Arc::new(MockMockMcpHandler::new()); // Use generated mock
        let mock_sse_state = Arc::new(TokioMutex::new(SseState::new(mock_mcp_handler.clone(), Arc::new(TokioMutex::new(McpPortState::default())))));
        // Use DashboardState::default() or a mock if needed
        let mock_dashboard_state = DashboardState::default(); 

        AppState::new(
            factory,
            mock_mcp_handler,
            mock_sse_state,
            mock_dashboard_state,
        )
    }

    #[actix_rt::test]
    async fn test_health_check() {
        // No state needed for health check
        let app = test::init_service(App::new().service(health_check)).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success(), "Health check failed");
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body, json!({ "status": "OK" }));
    }

    // Example test for get_emails (requires mock state)
    #[actix_rt::test]
    async fn test_get_emails_success() {
        let mock_state = create_mock_app_state();
        
        // Setup specific expectations for this test if needed
        // e.g., mock_state.imap_session_factory().expect...()

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(mock_state))
                .service(web::scope("/api/v1/accounts/{account}").service(get_emails))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/v1/accounts/test_account/emails?folder=INBOX")
            .to_request();
            
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK, "Expected OK status for get_emails");
        // Add more assertions based on the expected JSON body from the mock session
        // let body: serde_json::Value = test::read_body_json(resp).await;
        // assert!(body["emails"].is_array());
        // assert_eq!(body["total"], 3); // Based on default mock expectation
    }

    // Add more tests for other endpoints using mock sessions/factories
    // Test cases should cover:
    // - Success scenarios
    // - Error scenarios (invalid input, IMAP errors like NotFound, Conflict)
    // - Pagination (limit, offset)
    // - Different query parameters (search, folder)
}
