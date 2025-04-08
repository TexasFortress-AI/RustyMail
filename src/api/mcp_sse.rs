use actix_web::{web, Responder, HttpResponse, Error};
use actix_web::rt::time::interval;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{info, warn, error, debug};
use futures_util::StreamExt;
use crate::api::rest::AppState;
use crate::api::mcp_stdio::error_codes;
use crate::api::mcp_stdio::map_mcp_error_to_jsonrpc;
use serde_json::json;
use crate::prelude::McpPortError;

#[derive(Deserialize, Serialize, Debug)]
struct SseCommandPayload {
    client_id: Option<usize>,
    command: String,
    params: serde_json::Value,
}

// Placeholder for SSE specific config
pub struct SseConfig {
    // Potential config like heartbeat interval
}

// Structure to hold SSE client state
pub(crate) struct SseClient {
    sender: mpsc::Sender<String>,
}

// Shared state for SSE adapter
pub struct SseState {
    pub(crate) clients: HashMap<usize, SseClient>,
    pub(crate) visitor_count: usize,
}

impl SseState {
    // Public constructor
    pub fn new() -> Self {
        SseState {
            clients: HashMap::new(),
            visitor_count: 0,
        }
    }

    // Add a client
    fn add_client(&mut self, tx: mpsc::Sender<String>) -> usize {
        self.visitor_count += 1;
        let id = self.visitor_count;
        self.clients.insert(id, SseClient { sender: tx });
        id
    }

    // Remove a client
    fn remove_client(&mut self, id: usize) {
        self.clients.remove(&id);
        info!("SSE Client {} disconnected/removed.", id);
    }
}

// SSE Adapter Structure
pub struct SseAdapter {
    // Removed unused fields
    // shared_state: Arc<TokioMutex<SseState>>,
    // rest_config: RestConfig, 
}

// SSE Adapter Implementation
impl SseAdapter {
    pub fn new(
        // Removed unused parameters
        // shared_state: Arc<TokioMutex<SseState>>,
        // rest_config: RestConfig,
    ) -> Self {
        SseAdapter { 
            // Removed unused fields
            // shared_state,
            // rest_config
        }
    }

    // Handler for establishing SSE connection
    async fn sse_connect_handler(state: web::Data<Arc<TokioMutex<SseState>>>) -> impl Responder {
        let (tx, rx) = mpsc::channel::<String>(10);
        
        let client_id = state.lock().await.add_client(tx.clone());
        info!("New SSE Client connected: {}", client_id);

        // Example: Send a welcome message with client ID
        let welcome_msg = format!("event: welcome\ndata: {{ \"clientId\": {} }}\n\n", client_id);
        if tx.send(welcome_msg).await.is_err() {
            state.lock().await.remove_client(client_id);
            return HttpResponse::InternalServerError().finish();
        }

        let stream = async_stream::stream! {
            let mut interval_stream = interval(Duration::from_secs(10));
            let mut message_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

            loop {
                tokio::select! {
                    // Send heartbeat every 10s
                    _ = interval_stream.tick() => {
                        if tx.send("event: heartbeat\ndata: {}\n\n".to_string()).await.is_err() {
                            break; 
                        }
                    }
                    // Yield messages received on the channel
                    maybe_msg = message_stream.next() => {
                        if let Some(msg) = maybe_msg {
                             yield std::result::Result::<_, Error>::Ok(actix_web::web::Bytes::from(msg));
                        } else {
                            break; // Channel closed
                        }
                    }
                    else => {
                        break;
                    }
                }
            }

            state.lock().await.remove_client(client_id);
        };

        HttpResponse::Ok()
            .insert_header(("Content-Type", "text/event-stream"))
            .insert_header(("Cache-Control", "no-cache"))
            .streaming(stream)
    }

     // Handler for receiving commands via POST
    async fn sse_command_handler(
        app_state: web::Data<AppState>,
        sse_state: web::Data<Arc<TokioMutex<SseState>>>,
        payload: web::Json<SseCommandPayload>
    ) -> impl Responder
    {
        info!("Received SSE command for client {:?}: {} params: {}", payload.client_id, payload.command, payload.params);
        
        let client_id = match payload.client_id {
            Some(id) => id,
            None => {
                warn!("SSE command received without client_id");
                return HttpResponse::BadRequest().json(json!({ "error": "Missing client_id" }));
            }
        };

        let tool_name = payload.command.clone();
        let params = payload.params.clone();

        // --- Get Tool Registry --- 
        let tool_registry = &app_state.tool_registry;

        // --- Lookup and Execute Tool --- 
        let execution_result = match tool_registry.get(&tool_name) {
            Some(tool) => {
                debug!("Executing tool '{}' for SSE client {}", tool_name, client_id);
                tool.execute(params).await
            }
            None => {
                warn!("Tool '{}' not found for SSE client {}", tool_name, client_id);
                Err(McpPortError::NotImplemented(format!("Method not found: {}", tool_name)))
            }
        };

        // --- Format Result/Error as SSE Event --- 
        let sse_event_string: String;
        match execution_result {
            Ok(result_value) => {
                match serde_json::to_string(&result_value) {
                    Ok(result_json) => {
                        info!("Tool '{}' succeeded for SSE client {}. Sending result.", tool_name, client_id);
                        sse_event_string = format!("event: tool_result\ndata: {}

", result_json);
                    }
                    Err(e) => {
                        error!("Failed to serialize successful result for tool '{}', client {}: {}", tool_name, client_id, e);
                        // Directly create an internal error string for serialization failure
                        let err_msg = format!("Internal Server Error: Failed to serialize result: {}", e);
                        let err_json_str = ::serde_json::json!({ "code": error_codes::INTERNAL_ERROR, "message": err_msg }).to_string();
                        sse_event_string = format!("event: tool_error\ndata: {}

", err_json_str);
                    }
                }
            }
            Err(mcp_err) => {
                // Handle McpPortError correctly
                let (code, message) = map_mcp_error_to_jsonrpc(mcp_err);
                if code == error_codes::INTERNAL_ERROR || code < -32000 { // Log Internal and IMAP errors as ERROR
                    error!("Tool '{}' failed for SSE client {}: [{}] {}", tool_name, client_id, code, message);
                } else { // Log InvalidParams, NotFound etc. as WARN
                    warn!("Tool '{}' failed for SSE client {}: [{}] {}", tool_name, client_id, code, message);
                }
                // Create JSON-RPC error object
                 let err_json_str = ::serde_json::json!({ "code": code, "message": message }).to_string();
                sse_event_string = format!("event: tool_error\ndata: {}

", err_json_str);
            }
        }

        // --- Send Targeted Response via SSE --- 
        let state_guard = sse_state.lock().await;
        if let Some(client) = state_guard.clients.get(&client_id) {
            if client.sender.send(sse_event_string.clone()).await.is_err() {
                warn!("Failed to send SSE response to client {} (likely disconnected).", client_id);
                // Optionally remove client here, but connect_handler also handles cleanup
            } else {
                 debug!("Sent SSE response to client {}: {}", client_id, sse_event_string.trim());
            }
        } else {
            warn!("Could not find SSE client {} to send response.", client_id);
        }
        // Lock is released when state_guard goes out of scope

        // Respond to the POST request itself
        HttpResponse::Accepted().json(json!({ "status": "Command received and processed" }))
    }

    // Function to configure SSE routes within an Actix App
    pub fn configure_sse_service(cfg: &mut web::ServiceConfig) {
         // sse_state and app_state (containing tool registry) are already added in main.rs
         // Use cfg.app_data::<web::Data<AppState>>() if needed, but handlers access it directly
         cfg.service(
             web::scope("/api/v1/sse") // Group SSE routes
                 // sse_connect_handler only needs SseState, which is already in app_data
                 .route("/connect", web::get().to(Self::sse_connect_handler))
                 // sse_command_handler now uses AppState and SseState from app_data
                 .route("/command", web::post().to(Self::sse_command_handler)) // Endpoint for commands
         );
    }

    // Standalone server function (if SSE runs on its own port)
    // pub async fn run_sse_server(...) -> std::io::Result<()> { ... }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, http::{header, StatusCode}};
    use serde_json::{json, Value};
    use crate::api::rest::AppState;
    use tokio::sync::Mutex as TokioMutex;
    use crate::prelude::McpTool;

    // --- Mock Tools for SSE --- 
    struct MockSseSuccessTool;
    #[async_trait::async_trait]
    impl McpTool for MockSseSuccessTool {
        fn name(&self) -> &'static str { "sse/success" }
        fn description(&self) -> &'static str { "SSE success mock" }
        fn input_schema(&self) -> &'static str { "{}" }
        fn output_schema(&self) -> &'static str { "{}" }
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            Ok(json!({ "sse_ok": true, "params": params }))
        }
    }

    struct MockSseFailureTool;
    #[async_trait::async_trait]
    impl McpTool for MockSseFailureTool {
        fn name(&self) -> &'static str { "sse/fail" }
        fn description(&self) -> &'static str { "SSE failure mock" }
        fn input_schema(&self) -> &'static str { "{}" }
        fn output_schema(&self) -> &'static str { "{}" }
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
            Err(McpPortError::ToolError("SSE mock tool failed".to_string()))
        }
    }

    // --- Test Setup Helper ---
    async fn setup_test_app() -> (impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>, Arc<TokioMutex<SseState>>, Arc<HashMap<String, Arc<dyn McpTool>>>) {
        // Create shared SSE state
        let sse_state = Arc::new(TokioMutex::new(SseState {
            clients: HashMap::new(),
            visitor_count: 0,
        }));

        // Create mock tool registry
        let mut tool_registry_map: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
        tool_registry_map.insert("sse/success".to_string(), Arc::new(MockSseSuccessTool));
        tool_registry_map.insert("sse/fail".to_string(), Arc::new(MockSseFailureTool));
        let tool_registry = Arc::new(tool_registry_map);

        // --- Create a placeholder ImapClient --- 
        struct DummySession;
        #[async_trait::async_trait]
        impl crate::imap::session::ImapSession for DummySession {
            // Implement required methods with dummy behavior
            async fn list_folders(&self) -> Result<Vec<crate::imap::types::Folder>, crate::imap::error::ImapError> { unimplemented!() }
            async fn create_folder(&self, _name: &str) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn delete_folder(&self, _name: &str) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn rename_folder(&self, _from: &str, _to: &str) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn select_folder(&self, _name: &str) -> Result<crate::imap::types::MailboxInfo, crate::imap::error::ImapError> { unimplemented!() }
            async fn search_emails(&self, _criteria: crate::imap::types::SearchCriteria) -> Result<Vec<u32>, crate::imap::error::ImapError> { unimplemented!() }
            async fn fetch_emails(&self, _uids: Vec<u32>, _fetch_body: bool) -> Result<Vec<crate::imap::types::Email>, crate::imap::error::ImapError> { unimplemented!() }
            async fn move_email(&self, _source_folder: &str, _uids: Vec<u32>, _destination: &str) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn store_flags(&self, _uids: Vec<u32>, _operation: crate::imap::session::StoreOperation, _flags: Vec<String>) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn append(&self, _folder: &str, _payload: Vec<u8>) -> Result<(), crate::imap::error::ImapError> { unimplemented!() }
            async fn expunge(&self) -> Result<(), crate::imap::error::ImapError> { unimplemented!() } 
            // Fix logout signature: &self instead of Box<Self>
            async fn logout(&self) -> Result<(), crate::imap::error::ImapError> { unimplemented!() } 
            // Fix fetch_raw_message signature: &mut self instead of &self
            async fn fetch_raw_message(&mut self, _uid: u32) -> Result<Vec<u8>, crate::imap::error::ImapError> { unimplemented!() } 
        }
        let placeholder_imap_client = Arc::new(crate::imap::client::ImapClient::new_with_session(Arc::new(tokio::sync::Mutex::new(DummySession))));
        // --- End Placeholder ImapClient ---

        // Create AppState
        let app_state = AppState::new(placeholder_imap_client.clone(), tool_registry.clone());

        // Initialize Actix app for testing
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state.clone())) // Pass AppState
                .app_data(web::Data::new(sse_state.clone()))   // Pass SseState
                .configure(SseAdapter::configure_sse_service) // Configure SSE routes
        ).await;
        
        (app, sse_state, tool_registry) // Return components needed by tests
    }

    // --- Test Cases ---

    #[actix_web::test]
    async fn test_sse_connect_and_welcome() {
        let (app, _sse_state, _registry) = setup_test_app().await;
        let req = test::TestRequest::get().uri("/api/v1/sse/connect").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        assert_eq!(resp.headers().get(header::CONTENT_TYPE).unwrap(), "text/event-stream");
        // DO NOT read the body here, as it's a potentially infinite stream
        // Simply checking headers and status confirms the connection was accepted.
        // let body_bytes = test::read_body(resp).await;
        // let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        // println!("SSE Body: {}", body_str);
        // assert!(body_str.contains("event: welcome\ndata: { \"clientId\": 1 }\n\n"));
    }

    #[actix_web::test]
    async fn test_sse_command_post_accepted() {
        let (app, sse_state_arc, _registry) = setup_test_app().await;

        // 1. Connect client (get ID, but don't read stream here)
        let connect_req = test::TestRequest::get().uri("/api/v1/sse/connect").to_request();
        let _resp = test::call_service(&app, connect_req).await; // Consume response
        // Assume client ID 1 based on setup
        let client_id: usize = 1;
        // Ensure client is registered
        let sse_state = sse_state_arc.lock().await;
        assert!(sse_state.clients.contains_key(&client_id));
        drop(sse_state);

        // 2. Send command via POST
        let command_payload = json!({
            "client_id": client_id,
            "command": "sse/success",
            "params": { "arg": "test" }
        });
        let cmd_req = test::TestRequest::post().uri("/api/v1/sse/command").set_json(&command_payload).to_request();
        let cmd_resp = test::call_service(&app, cmd_req).await;
        // 3. Assert POST response is Accepted
        assert_eq!(cmd_resp.status(), StatusCode::ACCEPTED);
        let body = test::read_body(cmd_resp).await;
        let status_json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(status_json, json!({ "status": "Command received and processed" }));
        
        // We cannot easily assert the SSE message content here without robust stream reading
    }

     #[actix_web::test]
    async fn test_sse_command_tool_error_post_accepted() {
        let (app, sse_state_arc, _registry) = setup_test_app().await;
        let connect_req = test::TestRequest::get().uri("/api/v1/sse/connect").to_request();
        let _resp = test::call_service(&app, connect_req).await;
        let client_id: usize = 1;
        let sse_state = sse_state_arc.lock().await;
        assert!(sse_state.clients.contains_key(&client_id));
        drop(sse_state);

        let command_payload = json!({
            "client_id": client_id,
            "command": "sse/fail",
            "params": null
        });
        let cmd_req = test::TestRequest::post().uri("/api/v1/sse/command").set_json(&command_payload).to_request();
        let cmd_resp = test::call_service(&app, cmd_req).await;
        assert_eq!(cmd_resp.status(), StatusCode::ACCEPTED);
        // Cannot easily assert SSE error message content here
    }
    
    #[actix_web::test]
    async fn test_sse_command_method_not_found_post_accepted() {
         let (app, sse_state_arc, _registry) = setup_test_app().await;
         let connect_req = test::TestRequest::get().uri("/api/v1/sse/connect").to_request();
         let _resp = test::call_service(&app, connect_req).await;
         let client_id: usize = 1;
         let sse_state = sse_state_arc.lock().await;
         assert!(sse_state.clients.contains_key(&client_id));
         drop(sse_state);

         let command_payload = json!({
             "client_id": client_id,
             "command": "sse/nonexistent",
             "params": {}
         });
         let cmd_req = test::TestRequest::post().uri("/api/v1/sse/command").set_json(&command_payload).to_request();
         let cmd_resp = test::call_service(&app, cmd_req).await;
         assert_eq!(cmd_resp.status(), StatusCode::ACCEPTED);
         // Cannot easily assert SSE error message content here
    }
    
    #[actix_web::test]
    async fn test_sse_command_missing_client_id() {
        let (app, _sse_state_arc, _registry) = setup_test_app().await;
        let command_payload = json!({
            // No client_id field
            "command": "sse/success",
            "params": {}
        });
        let cmd_req = test::TestRequest::post().uri("/api/v1/sse/command").set_json(&command_payload).to_request();
        let cmd_resp = test::call_service(&app, cmd_req).await;
        assert_eq!(cmd_resp.status(), StatusCode::BAD_REQUEST);
        let body = test::read_body(cmd_resp).await;
        let error_json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error_json, json!({ "error": "Missing client_id" }));
    }
} 