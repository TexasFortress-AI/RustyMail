// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{
    web::{self, Data, Json, Path, Query},
    get, post, delete, put, // Added PUT and DELETE
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_lab::middleware::from_fn as mw_from_fn;
use log::info; // Only keep info for now
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

// Crate-local imports
use crate::{ // Group crate imports
    api::{
        auth::{ApiKeyStore, ApiScope, simple_validate_api_key},
        errors::{ApiError}, // Use new error module
    },
    config::Settings,
    // dashboard::api::errors::ApiError as DashboardApiError, // Now handled in errors.rs
    dashboard::services::DashboardState,
    imap::{
        client::ImapClient,
        session::AsyncImapSessionWrapper, // Import session type
        types::{ // Import necessary IMAP types
            FlagOperation, Flags,
        },
    },
    mcp::handler::McpHandler,
    session_manager::{SessionManager, SessionManagerTrait}, // Import both the struct and trait
};

// Define AppState struct *once*
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mcp_handler: Arc<dyn McpHandler>,
    pub session_manager: Arc<SessionManager>,
    pub dashboard_state: Option<Arc<TokioMutex<DashboardState>>>,
    pub api_key_store: Arc<ApiKeyStore>,
}

// ApiError is now in the errors module and imported above

// Legacy conversion - keep for backward compatibility
impl From<String> for ApiError {
    fn from(err: String) -> Self {
        ApiError::InternalError { message: err }
    }
}

// DashboardApiError conversion is now in errors.rs


// --- Route Configuration ---

pub fn configure_rest_service(cfg: &mut web::ServiceConfig) {
    // Scope for authenticated IMAP operations
    cfg.service(
        web::scope("/api/v1")
            .wrap(mw_from_fn(simple_validate_api_key))
            // Folder operations
            .service(list_folders)
            .service(get_folder)
            .service(create_folder)
            .service(update_folder)
            .service(delete_folder)
            .service(select_folder)
            // Email operations
            .service(list_emails)
            .service(get_email)
            .service(create_email)
            .service(update_email_flags)
            .service(delete_email)
            .service(move_email)
            .service(search_emails)
            // Bulk operations
            .service(expunge_folder)
    );

    // API Key management endpoints (require admin scope)
    cfg.service(
        web::scope("/api/v1/auth")
            .wrap(mw_from_fn(simple_validate_api_key))
            .service(get_api_key_info)
            .service(create_api_key)
            .service(revoke_api_key)
            .service(list_api_keys)
    );

    // Dashboard routes are configured separately in the main server setup
}

// --- Helper Functions ---

// Helper to get an IMAP session for the account specified by API key
async fn get_session(state: &AppState, req: &HttpRequest) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ApiError> {
    // Get API key from headers
    let api_key = req.headers()
        .get("X-API-Key")
        .or_else(|| req.headers().get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .map(|s| {
            if s.starts_with("Bearer ") {
                &s[7..]
            } else {
                s
            }
        })
        .ok_or(ApiError::Unauthorized)?;

    // Get API key data from store
    let api_key_data = state.api_key_store.validate_key(api_key).await?;

    // Try to get existing session
    let session_result = state.session_manager.as_ref().get_session(&api_key_data.key).await;

    match session_result {
        Ok(session) => Ok(session),
        Err(_) => {
            // Create new session with stored IMAP credentials
            let creds = &api_key_data.imap_credentials;
            state.session_manager.as_ref().create_session(
                &api_key_data.key,
                &creds.username,
                &creds.password,
                &creds.server,
                creds.port,
            ).await
            .map_err(|e| ApiError::InternalError { message: format!("Failed to create session: {}", e) })
        }
    }
}

// --- Route Handlers ---

// === Folder Operations ===

#[get("/folders")]
async fn list_folders(state: Data<AppState>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /folders");
    let session = get_session(&state, &req).await?;
    let folders: Vec<String> = session.list_folders().await?;

    // Transform to proper REST response format
    let folder_objects: Vec<serde_json::Value> = folders.iter().map(|name| {
        serde_json::json!({
            "name": name,
            "delimiter": "/",
            "attributes": [],
        })
    }).collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "folders": folder_objects,
        "total": folders.len(),
    })))
}

#[get("/folders/{folder_name}")]
async fn get_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling GET /folders/{}", folder_name);
    let session = get_session(&state, &req).await?;

    // Verify folder exists
    let folders = session.list_folders().await?;
    if !folders.contains(&folder_name) {
        return Err(ApiError::FolderNotFound { folder: folder_name.clone() });
    }

    // Select folder - note that actual implementation returns ()
    let _ = session.select_folder(&folder_name).await?;

    // Return folder info (without mailbox stats for now)
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "name": folder_name,
        "delimiter": "/",
        "attributes": [],
    })))
}

#[post("/folders")]
async fn create_folder(state: Data<AppState>, req: HttpRequest, payload: Json<CreateFolderRequest>) -> Result<HttpResponse, ApiError> {
    info!("Handling POST /folders");

    // Validate request
    if payload.name.is_empty() {
        return Err(ApiError::MissingField { field: "name".to_string() });
    }

    let session = get_session(&state, &req).await?;
    session.create_folder(&payload.name).await?;

    Ok(HttpResponse::Created()
        .insert_header(("Location", format!("/api/v1/folders/{}", payload.name)))
        .json(serde_json::json!({
            "name": payload.name,
            "delimiter": "/",
            "message": "Folder created successfully"
        })))
}

#[derive(Deserialize)]
struct CreateFolderRequest {
    name: String,
    #[serde(default)]
    parent: Option<String>,
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
async fn update_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>, payload: Json<UpdateFolderRequest>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling PUT /folders/{}", folder_name);

    let session = get_session(&state, &req).await?;

    // Handle rename if new name provided
    if let Some(new_name) = &payload.name {
        if new_name.is_empty() {
            return Err(ApiError::MissingField { field: "name".to_string() });
        }
        session.rename_folder(&folder_name, new_name).await?;

        Ok(HttpResponse::Ok()
            .insert_header(("Location", format!("/api/v1/folders/{}", new_name)))
            .json(serde_json::json!({
                "name": new_name,
                "message": "Folder renamed successfully"
            })))
    } else {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "name": folder_name,
            "message": "No changes made"
        })))
    }
}

#[derive(Deserialize)]
struct UpdateFolderRequest {
    name: Option<String>,
}

#[post("/folders/{folder_name}/select")]
async fn select_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/select", folder_name);
    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "folder": folder_name,
        "status": "selected",
    })))
}

// === Email Operations ===

#[get("/folders/{folder_name}/emails")]
async fn list_emails(state: Data<AppState>, req: HttpRequest, path: Path<String>, query: Query<ListEmailsQuery>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling GET /folders/{}/emails", folder_name);

    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;

    // Default to fetching recent emails if no specific query
    let search_criteria = query.search.as_deref().unwrap_or("ALL");
    let uids = session.search_emails(search_criteria).await?;

    // Apply pagination
    let limit = query.limit.unwrap_or(50).min(100); // Max 100 emails per request
    let offset = query.offset.unwrap_or(0);

    let paginated_uids: Vec<u32> = uids.iter()
        .skip(offset)
        .take(limit)
        .copied()
        .collect();

    // Fetch email headers for the paginated results
    let emails = if !paginated_uids.is_empty() {
        session.fetch_emails(&paginated_uids).await?
    } else {
        vec![]
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "emails": emails,
        "total": uids.len(),
        "limit": limit,
        "offset": offset,
        "folder": folder_name,
    })))
}

#[derive(Deserialize)]
struct ListEmailsQuery {
    search: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[get("/folders/{folder_name}/emails/{uid}")]
async fn get_email(state: Data<AppState>, req: HttpRequest, path: Path<(String, u32)>) -> Result<HttpResponse, ApiError> {
    let (folder_name, uid) = path.into_inner();
    info!("Handling GET /folders/{}/emails/{}", folder_name, uid);

    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;

    let emails = session.fetch_emails(&vec![uid]).await?;

    if emails.is_empty() {
        return Err(ApiError::EmailNotFound { uid });
    }

    Ok(HttpResponse::Ok().json(&emails[0]))
}

#[post("/folders/{folder_name}/emails")]
async fn create_email(state: Data<AppState>, req: HttpRequest, path: Path<String>, payload: Json<CreateEmailRequest>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/emails", folder_name);

    let session = get_session(&state, &req).await?;

    // Decode base64 content
    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&payload.content)
        .map_err(|e| ApiError::InvalidFieldValue { field: "content".to_string(), reason: format!("Invalid base64: {}", e) })?;

    let flags: Vec<String> = payload.flags.as_ref()
        .map(|f| f.items.iter().map(|flag| flag.to_string()).collect())
        .unwrap_or_default();

    session.append(&folder_name, &content, &flags).await?;

    Ok(HttpResponse::Created()
        .json(serde_json::json!({
            "message": "Email appended successfully",
            "folder": folder_name,
        })))
}

#[derive(Deserialize)]
struct CreateEmailRequest {
    content: String, // Base64 encoded RFC822 message
    flags: Option<Flags>,
}

#[get("/emails/search")]
async fn search_emails(state: Data<AppState>, req: HttpRequest, query: Query<SearchEmailsQuery>) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /emails/search");

    let session = get_session(&state, &req).await?;

    // Select folder if specified, otherwise use INBOX
    let folder = query.folder.as_deref().unwrap_or("INBOX");
    let _ = session.select_folder(folder).await?;

    let search_criteria = query.q.as_deref().unwrap_or("ALL");
    let uids = session.search_emails(search_criteria).await?;

    // Apply pagination
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let paginated_uids: Vec<u32> = uids.iter()
        .skip(offset)
        .take(limit)
        .copied()
        .collect();

    let emails = if !paginated_uids.is_empty() {
        session.fetch_emails(&paginated_uids).await?
    } else {
        vec![]
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "results": emails,
        "total": uids.len(),
        "query": search_criteria,
        "folder": folder,
    })))
}

#[derive(Deserialize)]
struct SearchEmailsQuery {
    q: Option<String>, // Search query
    folder: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[put("/folders/{folder_name}/emails/{uid}")]
async fn update_email_flags(state: Data<AppState>, req: HttpRequest, path: Path<(String, u32)>, payload: Json<UpdateEmailRequest>) -> Result<HttpResponse, ApiError> {
    let (folder_name, uid) = path.into_inner();
    info!("Handling PUT /folders/{}/emails/{}", folder_name, uid);

    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;

    if let Some(flags) = &payload.flags {
        let flag_strings: Vec<String> = flags.items.iter().map(|f| f.to_string()).collect();
        let operation = payload.flag_operation.clone().unwrap_or(FlagOperation::Set);
        session.store_flags(&vec![uid], operation, &flag_strings).await?;
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "uid": uid,
        "folder": folder_name,
        "message": "Email updated successfully"
    })))
}

#[derive(Deserialize)]
struct UpdateEmailRequest {
    flags: Option<Flags>,
    flag_operation: Option<FlagOperation>,
}

#[delete("/folders/{folder_name}/emails/{uid}")]
async fn delete_email(state: Data<AppState>, req: HttpRequest, path: Path<(String, u32)>) -> Result<HttpResponse, ApiError> {
    let (folder_name, uid) = path.into_inner();
    info!("Handling DELETE /folders/{}/emails/{}", folder_name, uid);

    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;

    // Mark email as deleted
    session.store_flags(&vec![uid], FlagOperation::Add, &vec!["\\Deleted".to_string()]).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "uid": uid,
        "folder": folder_name,
        "message": "Email marked for deletion. Run expunge to permanently delete."
    })))
}


#[post("/folders/{folder_name}/emails/{uid}/move")]
async fn move_email(state: Data<AppState>, req: HttpRequest, path: Path<(String, u32)>, payload: Json<MoveEmailRequest>) -> Result<HttpResponse, ApiError> {
    let (from_folder, uid) = path.into_inner();
    info!("Handling POST /folders/{}/emails/{}/move", from_folder, uid);

    if payload.to_folder.is_empty() {
        return Err(ApiError::MissingField { field: "to_folder".to_string() });
    }

    let session = get_session(&state, &req).await?;
    session.move_email(uid, &from_folder, &payload.to_folder).await?;

    Ok(HttpResponse::Ok()
        .insert_header(("Location", format!("/api/v1/folders/{}/emails/{}", payload.to_folder, uid)))
        .json(serde_json::json!({
            "uid": uid,
            "from": from_folder,
            "to": payload.to_folder,
            "message": "Email moved successfully"
        })))
}

#[derive(Deserialize)]
struct MoveEmailRequest {
    to_folder: String,
}

// === API Key Management ===

#[get("/keys/current")]
async fn get_api_key_info(state: Data<AppState>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /auth/keys/current");

    // Get API key from headers
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    // Return key info without sensitive data
    let info = state.api_key_store.get_key_info(api_key).await?;

    Ok(HttpResponse::Ok().json(info))
}

#[post("/keys")]
async fn create_api_key(state: Data<AppState>, req: HttpRequest, payload: Json<CreateApiKeyRequest>) -> Result<HttpResponse, ApiError> {
    info!("Handling POST /auth/keys");

    // Check if requester has admin scope
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    if !state.api_key_store.has_scope(api_key, &ApiScope::Admin).await {
        return Err(ApiError::Unauthorized);
    }

    // Validate request
    if payload.name.is_empty() || payload.email.is_empty() {
        return Err(ApiError::ValidationFailed {
            message: "Missing required fields".to_string(),
            errors: vec![
                crate::api::errors::ValidationError {
                    field: "name".to_string(),
                    message: "Name is required".to_string(),
                    constraint: Some("required".to_string()),
                },
                crate::api::errors::ValidationError {
                    field: "email".to_string(),
                    message: "Email is required".to_string(),
                    constraint: Some("required".to_string()),
                },
            ]
        });
    }

    // Create new API key
    let new_key = state.api_key_store.create_api_key(
        payload.name.clone(),
        payload.email.clone(),
        payload.imap_credentials.clone(),
        payload.scopes.clone().unwrap_or_else(|| vec![
            ApiScope::ReadEmail,
            ApiScope::WriteEmail,
            ApiScope::ManageFolders,
        ]),
    ).await;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "api_key": new_key,
        "message": "API key created successfully",
        "warning": "Store this key securely. It cannot be retrieved again."
    })))
}

#[derive(Deserialize)]
struct CreateApiKeyRequest {
    name: String,
    email: String,
    imap_credentials: crate::api::auth::ImapCredentials,
    scopes: Option<Vec<ApiScope>>,
}

#[delete("/keys/{key}")]
async fn revoke_api_key(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let key_to_revoke = path.into_inner();
    info!("Handling DELETE /auth/keys/{}", key_to_revoke);

    // Check if requester has admin scope
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    if !state.api_key_store.has_scope(api_key, &ApiScope::Admin).await {
        return Err(ApiError::Unauthorized);
    }

    // Don't allow self-revocation
    if api_key == key_to_revoke {
        return Err(ApiError::BadRequest { message: "Cannot revoke your own API key".to_string() });
    }

    state.api_key_store.revoke_key(&key_to_revoke).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "API key revoked successfully"
    })))
}

#[get("/keys")]
async fn list_api_keys(state: Data<AppState>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    info!("Handling GET /auth/keys");

    // Check if requester has admin scope
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    if !state.api_key_store.has_scope(api_key, &ApiScope::Admin).await {
        return Err(ApiError::Unauthorized);
    }

    // This would need to be implemented in ApiKeyStore
    // For now, return empty list
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "keys": [],
        "message": "API key listing not yet implemented"
    })))
}

// === Bulk Operations ===

#[post("/folders/{folder_name}/expunge")]
async fn expunge_folder(state: Data<AppState>, req: HttpRequest, path: Path<String>) -> Result<HttpResponse, ApiError> {
    let folder_name = path.into_inner();
    info!("Handling POST /folders/{}/expunge", folder_name);

    let session = get_session(&state, &req).await?;
    let _ = session.select_folder(&folder_name).await?;
    let _ = session.expunge().await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "folder": folder_name,
        "message": "Expunge operation completed successfully"
    })))
}

// --- Main Server Setup (Optional, if this is the main entry point) ---

pub async fn run_server(settings: Settings, mcp_handler: Arc<dyn McpHandler>, session_manager: Arc<SessionManager>, dashboard_state: Option<Arc<TokioMutex<DashboardState>>>) -> std::io::Result<()> {
    // Get the REST config and construct bind address
    let rest_config = settings.rest.as_ref()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "REST config not found"))?;
    let bind_address = format!("{}:{}", rest_config.host, rest_config.port);
    info!("Starting REST API server at {}", bind_address);

    // Initialize API key store
    let api_key_store = Arc::new(ApiKeyStore::new());
    api_key_store.init_with_defaults().await;

    let app_state = Data::new(AppState {
        settings: Arc::new(settings),
        mcp_handler,
        session_manager,
        dashboard_state,
        api_key_store: Arc::clone(&api_key_store),
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

// Tests disabled - need to be refactored for current API
/*
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
*/
