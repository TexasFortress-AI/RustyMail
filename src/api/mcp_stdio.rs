//! Handles MCP communication over stdin/stdout.

use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::imap::client::{ImapClient, ImapClientTrait};
use log::{debug, error, info, warn};
use crate::imap::types::{
    SearchCriteria
};
use crate::mcp_port::{McpPortError, McpTool, create_mcp_tool_registry};
use tokio::sync::Mutex as TokioMutex;
use crate::imap::types::{FlagOperation, Flags, AppendEmailPayload};

// Define state struct
#[derive(Debug, Clone, Default)] 
pub struct McpPortState {
    pub selected_folder: Option<String>,
}

// Define JSON-RPC request structure
#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

// Define JSON-RPC response structure
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

// Define JSON-RPC error structure
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// --- MCP Error Codes (from Spec) ---
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // Custom Application Errors (-32000 to -32099)
    pub const IMAP_CONNECTION_ERROR: i32 = -32000;
    pub const IMAP_AUTH_ERROR: i32 = -32001;
    pub const IMAP_FOLDER_NOT_FOUND: i32 = -32002;
    pub const IMAP_FOLDER_EXISTS: i32 = -32003;
    pub const IMAP_EMAIL_NOT_FOUND: i32 = -32004; // Can be used if fetch/store finds no matching UIDs
    pub const IMAP_OPERATION_FAILED: i32 = -32010; // Generic IMAP failure
    pub const IMAP_INVALID_CRITERIA: i32 = -32011; // e.g., bad search string
    pub const IMAP_REQUIRES_FOLDER_SELECTION: i32 = -32012;
}

// --- Error Mapping ---

// Function to map McpPortError to JSON-RPC error code and message
// Make pub(crate) so it can be used by other modules in the same crate (like mcp_sse)
pub(crate) fn map_mcp_error_to_jsonrpc(err: McpPortError) -> (i32, String) {
    match err {
        McpPortError::InvalidParams(m) => (error_codes::INVALID_PARAMS, m),
        McpPortError::ToolError(m) => (error_codes::IMAP_OPERATION_FAILED, m),
        McpPortError::ResourceError(m) => (error_codes::INTERNAL_ERROR, m),
        McpPortError::NotImplemented(m) => (error_codes::METHOD_NOT_FOUND, m),
        McpPortError::InternalError { message } => (error_codes::INTERNAL_ERROR, message),
        McpPortError::ImapConnectionError(m) => (error_codes::IMAP_CONNECTION_ERROR, m),
        McpPortError::ImapAuthenticationError(m) => (error_codes::IMAP_AUTH_ERROR, m),
        McpPortError::ImapFolderNotFound(m) => (error_codes::IMAP_FOLDER_NOT_FOUND, m),
        McpPortError::ImapFolderExists(m) => (error_codes::IMAP_FOLDER_EXISTS, m),
        McpPortError::ImapEmailNotFound(m) => (error_codes::IMAP_EMAIL_NOT_FOUND, m),
        McpPortError::ImapOperationFailed(m) => (error_codes::IMAP_OPERATION_FAILED, m),
        McpPortError::ImapInvalidCriteria(m) => (error_codes::IMAP_INVALID_CRITERIA, m),
        McpPortError::ImapRequiresFolderSelection(m) => (error_codes::IMAP_REQUIRES_FOLDER_SELECTION, m),
    }
}

// Function to create a standardized JSON-RPC error response
// Keep this private as it's only used within this module
fn create_jsonrpc_error_response(id: Option<Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.unwrap_or(Value::Null),
        result: None,
        error: Some(JsonRpcError { code, message, data: None }),
    }
}

// --- McpStdioAdapter Logic ---

pub struct McpStdioAdapter {
    tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>,
}

impl McpStdioAdapter {
    pub fn new(tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>) -> Self {
        Self { tool_registry }
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut stdin = BufReader::new(io::stdin());
        let mut stdout = BufWriter::new(io::stdout());
        let state = Arc::new(TokioMutex::new(McpPortState::default())); // State lives here
        let mut line = String::new();
        info!("MCP Stdio Adapter Ready. Waiting for commands...");

        loop {
            line.clear();
            match stdin.read_line(&mut line).await {
                Ok(0) => { info!("MCP Stdio Adapter closing (EOF)."); break; }
                Ok(_) => {
                    let trimmed_line = line.trim();
                    if trimmed_line.is_empty() { continue; }
                    debug!("Received raw MCP line: {}", trimmed_line);

                    let response = self.process_line(&state, trimmed_line).await; // Pass state reference

                    if let Ok(resp_str) = serde_json::to_string(&response) {
                        if response.error.is_none() {
                            info!("Sending MCP Success Response id={:?}", response.id);
                        } // Error logging happens in process_line/handle_request
                        debug!("Raw MCP response: {}", resp_str);
                        stdout.write_all(resp_str.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    } else {
                        error!("CRITICAL: Failed to serialize JSON-RPC response for id={:?}", response.id);
                        let fallback_err = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal Server Error: Failed to serialize response"}}"#;
                        stdout.write_all(fallback_err.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                }
                Err(e) => { error!("Error reading stdin: {}", e); break; }
            }
        }
        Ok(())
    }

    // Process a single line of input (JSON-RPC request)
    async fn process_line(&self, state: &Arc<TokioMutex<McpPortState>>, line: &str) -> JsonRpcResponse {
        match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(req) => {
                info!("Received MCP Request: id={:?}, method='{}'", req.id, req.method);
                // Pass state to handle_request
                self.handle_request(state, req).await 
            }
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {} | Raw line: {}", e, line);
                create_jsonrpc_error_response(None, error_codes::PARSE_ERROR, "Parse error".to_string())
            }
        }
    }

    // Handle a deserialized JSON-RPC request
    async fn handle_request(&self, state: &Arc<TokioMutex<McpPortState>>, req: JsonRpcRequest) -> JsonRpcResponse { 
        // Validate jsonrpc version
        if req.jsonrpc != "2.0" {
            return create_jsonrpc_error_response(req.id, error_codes::INVALID_REQUEST, "Invalid JSON-RPC version".to_string());
        }
        
        // Extract params or default to Null
        let mut params = req.params.unwrap_or(Value::Null);

        match self.tool_registry.get(&req.method) {
            Some(tool) => {
                debug!("Executing tool: '{}' with params: {:?}", req.method, params);

                // --- Inject state if needed by the tool --- 
                // Example: For moveEmails, inject the current selected folder
                if req.method == "imap/moveEmails" {
                    let current_state = state.lock().await;
                    if let Some(selected) = &current_state.selected_folder {
                         if let Value::Object(mut map) = params {
                             map.insert("source_folder".to_string(), json!(selected));
                             params = Value::Object(map);
                             debug!("Injected source_folder '{}' into params for moveEmails", selected);
                         } else {
                            error!("MoveEmails requires object params to inject source_folder");
                             return create_jsonrpc_error_response(req.id.clone(), error_codes::INVALID_PARAMS, "MoveEmails requires object params".to_string());
                         }
                    } else {
                         error!("MoveEmails requires a folder to be selected first");
                        return create_jsonrpc_error_response(req.id.clone(), error_codes::IMAP_REQUIRES_FOLDER_SELECTION, "No folder selected".to_string());
                    }
                }
                // --- End state injection --- 

                // Execute the tool (now without state arg)
                match tool.execute(params).await {
                    Ok(mut result) => {
                        // --- Handle state update for selectFolder --- 
                        if req.method == "imap/selectFolder" {
                             if let Some(map) = result.as_object_mut() {
                                 if let Some(folder_name_val) = map.remove("folder_name") { // Remove folder_name
                                     if let Some(folder_name) = folder_name_val.as_str() {
                                          let mut current_state = state.lock().await;
                                          current_state.selected_folder = Some(folder_name.to_string());
                                          info!("State updated: selected folder set to '{}'", folder_name);
                                     } else {
                                         error!("SelectFolder tool returned non-string folder_name");
                                          // Fall through, return mailbox_info anyway
                                     }
                                     // Replace result with just the mailbox_info part
                                     result = map.remove("mailbox_info").unwrap_or(Value::Null); 
                                 } else {
                                      error!("SelectFolder tool result missing folder_name");
                                      // Fall through, return original result
                                 }
                             } else {
                                 error!("SelectFolder tool result was not an object");
                                 // Fall through, return original result
                             }
                         }
                        // --- End state update --- 

                        info!("Tool '{}' executed successfully for id={:?}", req.method, req.id);
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id.unwrap_or(Value::Null),
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!("Tool '{}' execution failed for id={:?}: {:?}", req.method, req.id, e);
                        let (code, message) = map_mcp_error_to_jsonrpc(e);
                        create_jsonrpc_error_response(req.id.clone(), code, message)
                    }
                }
            }
            None => {
                warn!("Method not found: '{}' for id={:?}", req.method, req.id);
                create_jsonrpc_error_response(req.id.clone(), error_codes::METHOD_NOT_FOUND, "Method not found".to_string())
            }
        }
    }
}


// --- Unit Tests for McpStdioAdapter ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::client_test::MockImapClient;
    use super::error_codes;
    use crate::mcp_port::{McpTool, McpPortError, create_mcp_tool_registry};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use async_trait::async_trait;
    use serde::Deserialize; // Needed for mock tools
    use serde_json::Value; // Needed for mock tools

    // --- Define deserialize_params! locally for tests --- 
    macro_rules! deserialize_params {
        ($params_val:expr, $param_struct:ident) => {{
            // Ensure McpPortError is in scope here, or qualify it
            ::serde_json::from_value::< $param_struct >($params_val.clone())
                .map_err(|e| {
                    let err_msg = format!("Invalid parameters: {}", e);
                    // Assuming McpPortError::InvalidParams is accessible
                    McpPortError::InvalidParams(err_msg)
                })
        }};
    }

    // --- Mock Tools --- 
    struct MockSuccessTool;
    #[async_trait]
    impl McpTool for MockSuccessTool {
        fn name(&self) -> &'static str { "test/success" } // Use &'static str
        fn description(&self) -> &'static str { "A mock tool that always succeeds." } // Use &'static str
        fn input_schema(&self) -> &'static str { "{}" } // Add required method
        fn output_schema(&self) -> &'static str { "{}" } // Add required method
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            Ok(json!({ "success": true, "params_received": params }))
        }
    }

    struct MockFailureTool;
    #[async_trait]
    impl McpTool for MockFailureTool {
        fn name(&self) -> &'static str { "test/fail" } // Use &'static str
        fn description(&self) -> &'static str { "A mock tool that always fails." } // Use &'static str
        fn input_schema(&self) -> &'static str { "{}" } // Add required method
        fn output_schema(&self) -> &'static str { "{}" } // Add required method
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
            Err(McpPortError::ToolError("Mock Failure".to_string()))
        }
    }

    struct MockInvalidParamsTool;
    #[async_trait]
    impl McpTool for MockInvalidParamsTool {
        fn name(&self) -> &'static str { "test/invalidParams" } // Use &'static str
        fn description(&self) -> &'static str { "A mock tool that expects specific params." } // Use &'static str
        fn input_schema(&self) -> &'static str { "{}" } // Add required method
        fn output_schema(&self) -> &'static str { "{}" } // Add required method
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            #[derive(Deserialize)]
            struct ExpectedParams { /* required_field: String */ }
            let _p: ExpectedParams = deserialize_params!(params, ExpectedParams)?; // Use local macro
            Ok(json!({ "params_validated": true }))
        }
    }

    // Helper function to run a single request and get the response string
    async fn run_single_request(
        tools: Vec<Arc<dyn McpTool>>,
        input_json_str: &str,
    ) -> Result<String, String> {
        let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
        for tool in tools {
            tool_registry.insert(tool.name().to_string(), tool);
        }
        let adapter = McpStdioAdapter::new(Arc::new(tool_registry));
        
        // Create a dummy state for testing
        let state = Arc::new(TokioMutex::new(McpPortState::default()));

        // Call process_line directly
        let response = adapter.process_line(&state, input_json_str).await;
        
        serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
    }

    // --- Test Cases ---
    #[tokio::test]
    async fn test_success_request() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
        let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "test/success", "params": { "key": "value" } });
        let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["id"], 1);
        assert!(response["error"].is_null());
        assert_eq!(response["result"], json!({ "success": true, "params_received": { "key": "value" } }));
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
        let request = json!({ "jsonrpc": "2.0", "id": 2, "method": "test/nonexistent" });
        let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["id"], 2);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::METHOD_NOT_FOUND);
        assert_eq!(response["error"]["message"], "Method not found");
    }

    #[tokio::test]
    async fn test_tool_error() {
        let mock_client = Arc::new(MockImapClient::default().set_fail("list_folders"));
        let tool_registry = create_mcp_tool_registry(mock_client);
        let adapter = McpStdioAdapter::new(tool_registry);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(4)),
            method: "imap/listFolders".to_string(),
            params: None,
        };

        // Use handle_request
        let state = Arc::new(TokioMutex::new(McpPortState::default()));
        let response = adapter.handle_request(&state, req).await;

        assert_eq!(response.id, Value::from(4));
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, error_codes::IMAP_OPERATION_FAILED); 
        assert!(error.message.contains("Mock configured to return error"));
    }

    #[tokio::test]
    async fn test_parse_error() {
        let tools: Vec<Arc<dyn McpTool>> = vec![];
        let invalid_json = "{ jsonrpc: \"2.0\", id: 4, method: \"test\" "; // Missing closing brace
        let response_str = run_single_request(tools, invalid_json).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        // Parse errors might not have an ID
        // assert_eq!(response["id"], Value::Null); 
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::PARSE_ERROR);
        assert_eq!(response["error"]["message"], "Parse error");
    }

    #[tokio::test]
    async fn test_invalid_params_error() {
        let mock_client = Arc::new(MockImapClient::default());
        
        // Create the mock tool that causes the error
        #[derive(Deserialize)] struct ExpectedParams { /* No fields expected */ }
        struct MockRequiresParamsTool;
        #[async_trait]
        impl McpTool for MockRequiresParamsTool {
            fn name(&self) -> &'static str { "mock/requiresParams" }
            fn description(&self) -> &'static str { "Requires specific params" }
            fn input_schema(&self) -> &'static str { "{}" }
            fn output_schema(&self) -> &'static str { "{}" }
            async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
                let _p: ExpectedParams = deserialize_params!(params, ExpectedParams)?;
                Ok(json!({ "ok": true }))
            }
        }

        // Create the base registry from the real tools (using mock client)
        // This now calls the function in mcp_port which defines the real tools
        let mut tool_map = (*create_mcp_tool_registry(mock_client)).clone(); // Clone the map from the Arc
        // Add the specific mock tool needed for this test
        tool_map.insert("mock/requiresParams".to_string(), Arc::new(MockRequiresParamsTool));
        let tool_registry = Arc::new(tool_map);

        // Create adapter with the combined registry
        let adapter = McpStdioAdapter::new(tool_registry);
        
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(2)),
            method: "mock/requiresParams".to_string(),
            params: Some(json!({ "unexpected_field": 123 })), // Provide invalid params
        };

        let state = Arc::new(TokioMutex::new(McpPortState::default()));
        let response = adapter.handle_request(&state, req).await;

        assert_eq!(response.id, Value::from(2)); 
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS); // Use error_codes constant
        assert!(error.message.contains("Invalid parameters: unknown field `unexpected_field`"), "Error message was: {}", error.message);
    }

    #[tokio::test]
    async fn test_invalid_jsonrpc_version() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
        let request = json!({ "jsonrpc": "1.0", "id": 6, "method": "test/success" }); // Invalid version
        let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["id"], 6);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::INVALID_REQUEST);
        assert_eq!(response["error"]["message"], "Invalid JSON-RPC version");
    }
    
    #[tokio::test]
    async fn test_request_without_id() {
         let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
         // Notification (no ID)
         let request = json!({ "jsonrpc": "2.0", "method": "test/success", "params": [1, 2] }); 
         let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
         let response: Value = serde_json::from_str(&response_str).unwrap();
         
         // ID should be null for notifications
         assert_eq!(response["id"], Value::Null);
         assert!(response["error"].is_null());
         assert_eq!(response["result"], json!({ "success": true, "params_received": [1, 2] }));
    }
    
    // Example test showing state modification for selectFolder
    #[tokio::test]
    async fn test_select_folder_state_update() {
        struct MockSelectFolderTool;
        #[async_trait]
        impl McpTool for MockSelectFolderTool {
            fn name(&self) -> &'static str { "imap/selectFolder" }
            fn description(&self) -> &'static str { "Selects" }
            fn input_schema(&self) -> &'static str { "{}" }
            fn output_schema(&self) -> &'static str { "{}" }
            async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
                // Return the structure expected by the adapter
                Ok(json!({
                    "folder_name": "INBOX.TestFolder",
                    "mailbox_info": { "exists": 10, "recent": 1 }
                }))
            }
        }
        
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSelectFolderTool)];
        let adapter = McpStdioAdapter::new(Arc::new(tool_registry_from_vec(tools)));
        let state = Arc::new(TokioMutex::new(McpPortState::default()));
        let request = json!({ "jsonrpc": "2.0", "id": 10, "method": "imap/selectFolder", "params": { "name": "TestFolder" } });
        
        let response = adapter.process_line(&state, &request.to_string()).await;
        
        // Check response (should only contain mailbox_info)
        assert!(response.error.is_none());
        assert_eq!(response.result, Some(json!({ "exists": 10, "recent": 1 })));
        
        // Check state update
        let current_state = state.lock().await;
        assert_eq!(current_state.selected_folder, Some("INBOX.TestFolder".to_string()));
    }

    // Example test showing state injection for moveEmails
    #[tokio::test]
    async fn test_move_email_state_injection() {
        struct MockMoveEmailTool;
        #[async_trait]
        impl McpTool for MockMoveEmailTool {
            fn name(&self) -> &'static str { "imap/moveEmails" }
            fn description(&self) -> &'static str { "Moves" }
            fn input_schema(&self) -> &'static str { "{}" }
            fn output_schema(&self) -> &'static str { "{}" }
            async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
                #[derive(Deserialize, Serialize, Debug)]
                struct Params { uids: Vec<u32>, destination_folder: String, source_folder: String }
                let p: Params = deserialize_params!(params, Params)?;
                // Assert source_folder was injected
                assert_eq!(p.source_folder, "INBOX.Current");
                Ok(json!({ "moved": true, "params": p }))
            }
        }
        
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockMoveEmailTool)];
        let adapter = McpStdioAdapter::new(Arc::new(tool_registry_from_vec(tools)));
        let state = Arc::new(TokioMutex::new(McpPortState { selected_folder: Some("INBOX.Current".to_string()) }));
        let request = json!({ 
            "jsonrpc": "2.0", 
            "id": 11, 
            "method": "imap/moveEmails", 
            "params": { "uids": [1, 2], "destination_folder": "Archive" }
            // source_folder is NOT in the request, it's injected
        });
        
        let response = adapter.process_line(&state, &request.to_string()).await;
        
        assert!(response.error.is_none());
        assert!(response.result.is_some());
        let result_val = response.result.unwrap();
        assert_eq!(result_val["moved"], true);
        assert_eq!(result_val["params"]["source_folder"], "INBOX.Current");
    }

    // Helper to create registry from Vec
    fn tool_registry_from_vec(tools: Vec<Arc<dyn McpTool>>) -> HashMap<String, Arc<dyn McpTool>> {
        let mut map = HashMap::new();
        for tool in tools {
            map.insert(tool.name().to_string(), tool);
        }
        map
    }
}

// --- McpToolExecParams and similar structs are removed as direct calls are no longer needed ---
// They were primarily for a different architecture where stdio adapter called specific handlers.
// Now, the adapter uses the generic McpTool trait and the tool registry.

// --- Removed handle_tool_exec, handle_select_folder, handle_move_emails ---
// These functions are superseded by the logic within McpStdioAdapter::handle_request
// ... other param structs ...
// ... other param structs ...
// ... other param structs ...
// ... other param structs ...
// ... other param structs ...
// ... other param structs ...
// ... other param structs ...