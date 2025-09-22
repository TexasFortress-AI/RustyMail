use actix_web::{
    web::{self, Data, Json, Path, Query},
    get, post, delete, put, // Added PUT and DELETE
    App, Error as ActixError, HttpRequest, HttpResponse, HttpServer, Responder,
    ResponseError,
    http::StatusCode,
};
use actix_web_lab::middleware::from_fn as mw_from_fn; // Keep if used for middleware
use log::{error, info, warn}; // Keep necessary log levels
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;

// Crate-local imports
use crate::{ // Group crate imports
    config::Settings,
    dashboard::api::errors::ApiError as DashboardApiError, // Use alias to avoid clash
    dashboard::services::DashboardState,
    imap::{
        client::ImapClient,
        error::ImapError,
        session::{AsyncImapOps, AsyncImapSessionWrapper}, // Import session types
        types::{ // Import necessary IMAP types
            AppendEmailPayload, FlagOperation, Flags, Folder, Email, MailboxInfo, ModifyFlagsPayload, 
            SearchCriteria,
        },
    },
    mcp::{
        handler::McpHandler,
        types::{McpPortState, JsonRpcResponse, JsonRpcRequest, JsonRpcError}, // Remove McpCommand/Result if unused here
    },
    session_manager::SessionManager, // Assuming this manages sessions
    // Remove unused imports like Uuid, HashMap, Future, if they aren't needed
};

// Define AppState struct *once*
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mcp_handler: Arc<dyn McpHandler>,
    pub session_manager: Arc<SessionManager>,
    pub dashboard_state: Option<Arc<TokioMutex<DashboardState>>>,
}

// --- Error Handling ---
#[derive(Debug, Error, Serialize)] // Ensure ApiError derives Serialize
pub enum ApiError {
    #[error("IMAP Error: {0}")]
    Imap(String),

    #[error("MCP Error: {0}")]
    Mcp(String),

    #[error("Authentication Required")]
    Unauthorized,

    #[error("Invalid Request: {0}")]
    BadRequest(String),

    #[error("Resource Not Found: {0}")]
    NotFound(String),

    #[error("Internal Server Error: {0}")]
    InternalError(String),

    #[error("Dashboard Error: {0}")]
    Dashboard(String),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Imap(_) | ApiError::Mcp(_) | ApiError::InternalError(_) | ApiError::Dashboard(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        error!("API Error: {}", self);
        HttpResponse::build(self.status_code())
            .json(JsonRpcError::server_error(
                self.status_code().as_u16() as i64, // Use status code as error code
                self.to_string(), // Use the error message
                None,
            ))
    }
}

// Convert ImapError to ApiError
impl From<ImapError> for ApiError {
    fn from(err: ImapError) -> Self {
        ApiError::Imap(err.to_string())
    }
}

// Convert DashboardApiError to ApiError
impl From<DashboardApiError> for ApiError {
    fn from(err: DashboardApiError) -> Self {
        ApiError::Dashboard(err.to_string())
    }
}

// --- Middleware ---

use actix_web::dev::ServiceRequest;
use actix_web_lab::middleware::Next;

/// Simple API key validation middleware
async fn validate_api_key(
    req: ServiceRequest,
    next: Next<impl actix_web::body::MessageBody>,
) -> Result<actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>, ActixError> {
    // Check for API key in header
    let has_api_key = req.headers()
        .get("X-API-Key")
        .or_else(|| req.headers().get("Authorization"))
        .is_some();

    if !has_api_key {
        warn!("Request missing API key");
        return Err(ActixError::from(ApiError::InvalidApiKey("Missing API key".to_string())));
    }

    // For now, just check presence - in production, validate the actual key
    next.call(req).await
}

// --- Route Configuration ---

pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
    // Scope for authenticated IMAP operations
    cfg.service(
        web::scope("/api/v1")
            .wrap(mw_from_fn(validate_api_key))
            .service(list_folders)
            // ... other IMAP routes ...
            .service(create_folder)
            .service(delete_folder)
            .service(rename_folder)
            .service(select_folder)
            .service(search_emails)
            .service(fetch_emails)
            .service(modify_flags)
            .service(move_email)
            .service(append_email)
            .service(expunge_folder)
    );

    // Optionally configure dashboard routes if dashboard feature is enabled
    #[cfg(feature = "dashboard")]
    {
        use crate::dashboard::api::configure_dashboard_routes;
        cfg.service(web::scope("/dashboard").configure(configure_dashboard_routes));
    }
}

// --- Helper Functions ---

// Helper to get an IMAP session for the account specified by API key
async fn get_session(state: &AppState, req: &HttpRequest) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ApiError> {
    let api_key = req.headers().get("X-API-Key")
        .and_then(|hv| hv.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    state.session_manager.as_ref().get_session(api_key)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to get session: {}", e)))
}

// --- Route Handlers ---

#[get("/folders")]
async fn list_folders(state: Data<AppState>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /folders");
    let session = get_session(&state, &req).await?;
    let folders: Vec<String> = session.list_folders().await?; // Correct return type is Vec<String>
    Ok(HttpResponse::Ok().json(folders))
}

#[post("/folders")]
async fn create_folder(state: Data<AppState>, req: HttpRequest, payload: Json<Folder>) -> Result<HttpResponse, ApiError> {
    info!("Handling POST /folders");
    let session = get_session(&state, &req).await?;
    session.create_folder(&payload.name).await?;
    Ok(HttpResponse::Created().finish())
}

#[delete("/folders/{folder_name}")]
async fn delete_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling DELETE /folders/{}", folder_name);
    let session = get_session(&state, &req).await?;
    session.delete_folder(&folder_name).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[put("/folders/{folder_name}")]
async fn rename_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>, payload: Json<Folder>) -> Result<HttpResponse, ApiError> {
    let old_name = path.into_inner();
    let new_name = payload.into_inner().name;
    info!("Handling PUT /folders/{} -> {}", old_name, new_name);
    let session = get_session(&state, &req).await?;
    session.rename_folder(&old_name, &new_name).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/folders/{folder_name}/select")]
async fn select_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/select", folder_name);
    let session = get_session(&state, &req).await?;
    let mailbox_info: MailboxInfo = session.select_folder(&folder_name).await?; // Assuming select_folder returns MailboxInfo
    Ok(HttpResponse::Ok().json(mailbox_info))
}

#[get("/folders/{folder_name}/emails")]
async fn search_emails(state: Data<AppState>, req: HttpRequest, path: Path<String>, query: Query<SearchCriteria>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling GET /folders/{}/emails with criteria: {}", folder_name, query.to_string());
    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?; // Select folder first
    let uids = session.search_emails(&query.to_string()).await?;
    Ok(HttpResponse::Ok().json(uids))
}

#[get("/emails")]
async fn fetch_emails(state: Data<AppState>, req: HttpRequest, query: Query<FetchEmailsQuery>) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /emails?uids={}", query.uids);
    let session = get_session(&state, &req).await?;
    let uids: Vec<u32> = query.uids.split(',') // Parse UIDs from query string
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if uids.is_empty() {
        return Err(ApiError::BadRequest("No valid UIDs provided".to_string()));
    }
    let emails = session.fetch_emails(&uids).await?; // Pass Vec<u32>
    Ok(HttpResponse::Ok().json(emails))
}

#[derive(Deserialize)]
struct FetchEmailsQuery {
    uids: String, // Comma-separated UIDs
}

#[post("/emails/flags")]
async fn modify_flags(state: Data<AppState>, req: HttpRequest, payload: Json<ModifyFlagsPayload>) -> Result<HttpResponse, ApiError> {
    info!("Handling POST /emails/flags");
    let session = get_session(&state, &req).await?;
    let uids = &payload.uids;
    let operation = payload.operation.clone(); // Clone operation
    let flags_to_modify: Vec<String> = payload.flags.items.iter().map(|f| f.to_string()).collect(); // Convert flags to Vec<String>

    session.store_flags(uids, operation, &flags_to_modify).await?; // Pass Vec<String>
    Ok(HttpResponse::Ok().finish())
}

#[post("/emails/move")]
async fn move_email(state: Data<AppState>, req: HttpRequest, payload: Json<MoveEmailPayload>) -> Result<HttpResponse, ApiError> {
    info!("Handling POST /emails/move");
    let session = get_session(&state, &req).await?;
    // Assuming MoveEmailPayload contains uid, from_folder, to_folder
    session.move_email(payload.uid, &payload.from_folder, &payload.to_folder).await?;
    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct MoveEmailPayload {
    uid: u32,
    from_folder: String,
    to_folder: String,
}

#[post("/folders/{folder_name}/emails/append")]
async fn append_email(state: Data<AppState>, req: HttpRequest, path: Path<String>, payload: Json<AppendEmailPayload>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/emails/append", folder_name);
    let session = get_session(&state, &req).await?;
    // Decode base64 content
    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&payload.content)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 content: {}", e)))?;
    let flags: Vec<String> = payload.flags.items.iter().map(|f| f.to_string()).collect(); // Convert flags

    session.append(&folder_name, &content, &flags).await?;
    Ok(HttpResponse::Created().finish())
}

#[post("/folders/{folder_name}/expunge")]
async fn expunge_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/expunge", folder_name);
    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?; // Select folder first
    session.expunge().await?;
    Ok(HttpResponse::Ok().finish())
}

// --- Main Server Setup (Optional, if this is the main entry point) ---

pub async fn run_server(settings: Settings, mcp_handler: Arc<dyn McpHandler>, session_manager: Arc<SessionManager>, dashboard_state: Option<Arc<TokioMutex<DashboardState>>>) -> std::io::Result<()> {
    let bind_address = settings.interface.rest_bind_address();
    info!("Starting REST API server at {}", bind_address);

    let app_state = Data::new(AppState {
        settings: Arc::new(settings),
        mcp_handler,
        session_manager,
        dashboard_state,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .configure(configure_rest_service)
    })
    .bind(bind_address)?
    .run()
    .await
}

// --- Tests (Example Structure) ---

#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module
    use actix_web::{test, App}; // Minimal test imports
    use crate::imap::client::ImapClient;
    use crate::imap::error::ImapError; 
    use crate::imap::session::{AsyncImapOps, AsyncImapSessionWrapper}; // Use correct path
    use crate::imap::types::{Email, Folder, MailboxInfo, SearchCriteria}; // Import necessary types for mocking/assertions
    use crate::mcp::handler::MockMcpHandler; // Assume MockMcpHandler exists
    use crate::session_manager::MockSessionManager; // Assume MockSessionManager exists
    use mockall::{
        automock,
        predicate::*
    };
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;

    // Mock the AsyncImapOps trait if needed for direct IMAP interaction tests
    #[automock]
    #[async_trait::async_trait] // Add async_trait here as well
    pub trait MockAsyncImapOpsTrait: AsyncImapOps + Send + Sync + std::fmt::Debug {}
    // Implement the marker trait for the mock struct
    impl MockAsyncImapOpsTrait for crate::imap::client_test::tests::MockAsyncImapOps {}

    // Helper to create mock AppState
    fn create_mock_app_state() -> AppState {
        // Setup necessary mocks (McpHandler, SessionManager, etc.)
        let mut mock_mcp_handler = MockMcpHandler::new();
        // Set expectations on mock_mcp_handler if needed

        let mut mock_session_manager = MockSessionManager::new();
        // Example: Expect get_session to be called and return a mock ImapClient
        mock_session_manager.expect_get_session()
            .returning(|_api_key| {
                let mut mock_imap_ops = crate::imap::client_test::tests::MockAsyncImapOps::new();
                // Set expectations on mock_imap_ops for the specific test
                mock_imap_ops.expect_list_folders().returning(|| Ok(vec!["INBOX".to_string()]));
                // ... other expectations
                let client = ImapClient::new(mock_imap_ops);
                Ok(Arc::new(client))
            });

        AppState {
            settings: Arc::new(Settings::default_test()), // Assuming a test helper for Settings
            mcp_handler: Arc::new(mock_mcp_handler),
            session_manager: Arc::new(mock_session_manager),
            dashboard_state: None, // Or mock dashboard state if needed
        }
    }

    #[actix_web::test]
    async fn test_list_folders_route_success() {
        let app_state = create_mock_app_state();
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state))
                .configure(configure_rest_service)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/v1/folders")
            .insert_header(("X-API-Key", "test-key")) // Add API key
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: Vec<String> = test::read_body_json(resp).await;
        assert_eq!(body, vec!["INBOX".to_string()]);
    }

    // Add more tests for other routes (create_folder, errors, etc.)
}
