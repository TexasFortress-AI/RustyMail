use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::imap::client::ImapClient;
use log::{debug, error, info, warn};
use crate::imap::types::{
    SearchCriteria, ModifyFlagsPayload,
    AppendEmailPayload, ExpungeResponse,
};
use crate::mcp_port::{McpPortError, McpTool};

// --- JSON-RPC Structures (Compliant with Spec) ---

#[derive(Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
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
fn map_mcp_error_to_jsonrpc(err: McpPortError) -> (i32, String) {
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
fn create_jsonrpc_error_response(id: Option<Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.unwrap_or(Value::Null),
        result: None,
        error: Some(JsonRpcError { code, message, data: None }),
    }
}

// --- MCP Tool Implementations (using McpPortError from mcp_port) ---

// Macro to reduce boilerplate for tool struct and name/description
macro_rules! mcp_tool {
    ($struct_name:ident, $client_field:ident, $tool_name:expr, $tool_desc:expr, async fn execute($self:ident, $params:ident : Value) -> Result<Value, McpPortError> $body:block) => {
        pub struct $struct_name {
            $client_field: Arc<ImapClient>,
        }
        impl $struct_name {
             pub fn new(client: Arc<ImapClient>) -> Self { Self { $client_field: client } }
        }
        #[async_trait]
        impl McpTool for $struct_name {
            fn name(&$self) -> &str { $tool_name }
            fn description(&$self) -> &str { $tool_desc }
            async fn execute(&$self, $params: Value) -> Result<Value, McpPortError> $body
        }
    };
}

// Macro to deserialize params or return InvalidParams error
macro_rules! deserialize_params {
    ($params:expr, $type:ty) => {
        serde_json::from_value::<$type>($params).map_err(|e| {
            McpPortError::InvalidParams(format!("Invalid parameters: {}", e))
        })?
    };
}

mcp_tool!(McpListFoldersTool, client, "imap/listFolders", "Lists all IMAP folders.",
    async fn execute(self, _params: Value) -> Result<Value, McpPortError> {
        let folders = self.client.list_folders().await?;
        Ok(serde_json::to_value(folders).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);

mcp_tool!(McpCreateFolderTool, client, "imap/createFolder", "Creates a new IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { name: String }
        let p: Params = deserialize_params!(params, Params);
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        self.client.create_folder(&p.name).await?;
        Ok(json!({ "message": "Folder created", "name": p.name }))
    }
);

mcp_tool!(McpDeleteFolderTool, client, "imap/deleteFolder", "Deletes an IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { name: String }
        let p: Params = deserialize_params!(params, Params);
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        self.client.delete_folder(&p.name).await?;
        Ok(json!({ "message": "Folder deleted", "name": p.name }))
    }
);

mcp_tool!(McpRenameFolderTool, client, "imap/renameFolder", "Renames an IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { from_name: String, to_name: String }
        let p: Params = deserialize_params!(params, Params);
        if p.from_name.trim().is_empty() || p.to_name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder names cannot be empty".into())); }
        self.client.rename_folder(&p.from_name, &p.to_name).await?;
        Ok(json!({ "message": "Folder renamed", "from_name": p.from_name, "to_name": p.to_name }))
    }
);

mcp_tool!(McpSelectFolderTool, client, "imap/selectFolder", "Selects a folder, making it active for subsequent commands.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { name: String }
        let p: Params = deserialize_params!(params, Params);
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        let mailbox_info = self.client.select_folder(&p.name).await?;
        Ok(serde_json::to_value(mailbox_info).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);

mcp_tool!(McpSearchEmailsTool, client, "imap/searchEmails", "Searches emails in the currently selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { criteria: SearchCriteria }
        let p: Params = deserialize_params!(params, Params);
        let uids = self.client.search_emails(p.criteria).await?;
        Ok(json!({ "uids": uids }))
    }
);

mcp_tool!(McpFetchEmailsTool, client, "imap/fetchEmails", "Fetches emails by UID from the selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { uids: Vec<u32>, fetch_body: Option<bool> }
        let p: Params = deserialize_params!(params, Params);
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        // Use fetch_body, defaulting to false if None
        let fetch_body = p.fetch_body.unwrap_or(false);
        let emails = self.client.fetch_emails(p.uids, fetch_body).await?;
        Ok(serde_json::to_value(emails).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);

mcp_tool!(McpMoveEmailTool, client, "imap/moveEmails", "Moves emails by UID from the selected folder to a destination folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { uids: Vec<u32>, destination_folder: String }
        let p: Params = deserialize_params!(params, Params);
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        if p.destination_folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Destination folder cannot be empty".into())); }
        self.client.move_email(p.uids.clone(), &p.destination_folder).await?;
        Ok(json!({ "message": "Emails moved", "uids": p.uids, "destination_folder": p.destination_folder }))
    }
);

mcp_tool!(McpStoreFlagsTool, client, "imap/storeFlags", "Adds, removes, or sets flags for specified emails in the selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        let p: ModifyFlagsPayload = deserialize_params!(params, ModifyFlagsPayload);
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        self.client.store_flags(p.uids, p.operation, p.flags).await?;
        Ok(json!({ "message": "Flags stored successfully" }))
    }
);

mcp_tool!(McpAppendEmailTool, client, "imap/appendEmail", "Appends an email message to the specified folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> {
        #[derive(Deserialize)] struct Params { folder: String, email: AppendEmailPayload }
        let p: Params = deserialize_params!(params, Params);
        if p.folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        let maybe_uid = self.client.append(&p.folder, p.email).await?;
        Ok(json!({ "message": "Email appended", "uid": maybe_uid }))
    }
);

mcp_tool!(McpExpungeFolderTool, client, "imap/expungeFolder", "Permanently removes emails marked \\\\Deleted from the selected folder.",
    async fn execute(self, _params: Value) -> Result<Value, McpPortError> {
        let response: ExpungeResponse = self.client.expunge().await?;
        Ok(serde_json::to_value(response).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);


// --- MCP Stdio Adapter (Updated Error Handling) ---

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

                    let response = self.process_line(trimmed_line).await;

                    if let Ok(resp_str) = serde_json::to_string(&response) {
                        // Log success only if it's not an error response
                        if response.error.is_none() {
                            info!("Sending MCP Success Response id={:?}", response.id);
                        } else {
                            // Error is logged within handle_request or process_line
                        }
                        debug!("Raw MCP response: {}", resp_str);
                        stdout.write_all(resp_str.as_bytes()).await?;
                        stdout.write_all(b"\\n").await?; // Ensure newline termination
                        stdout.flush().await?;
                    } else {
                        // This case should ideally not happen if JsonRpcResponse serialization is correct
                        error!("CRITICAL: Failed to serialize even a basic error response for id={:?}", response.id);
                        // Send a minimal, hardcoded error if possible
                        let fallback_err = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal Server Error: Failed to serialize response"}}"#;
                        stdout.write_all(fallback_err.as_bytes()).await?;
                        stdout.write_all(b"\\n").await?;
                        stdout.flush().await?;
                    }
                }
                Err(e) => { error!("Error reading stdin: {}", e); break; }
            }
        }
        Ok(())
    }

    async fn process_line(&self, line: &str) -> JsonRpcResponse {
        match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(req) => {
                if req.jsonrpc != "2.0" {
                    let err_msg = "Invalid jsonrpc version, must be '2.0'".to_string();
                    warn!("Invalid Request: {} (id: {:?})", err_msg, req.id);
                    return create_jsonrpc_error_response(req.id, error_codes::INVALID_REQUEST, err_msg);
                }
                self.handle_request(req).await
            }
            Err(e) => {
                // Attempt to extract ID even from invalid JSON for better error reporting
                let id = serde_json::from_str::<Value>(line)
                    .ok()
                    .and_then(|v| v.get("id").cloned());
                let err_msg = format!("Parse error: {}", e);
                error!("MCP Parse Error: {} - Raw line: '{}'", err_msg, line);
                // Use PARSE_ERROR code for JSON parsing issues
                create_jsonrpc_error_response(id, error_codes::PARSE_ERROR, err_msg)
            }
        }
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let request_id = req.id.clone();
        let method_name = req.method.clone(); // Clone for logging on error

        match self.tool_registry.get(&req.method) {
            Some(tool) => {
                let params = req.params.unwrap_or(Value::Null);
                debug!("Executing tool '{}' for id={:?} with params: {}", req.method, request_id, params);
                match tool.execute(params).await {
                    Ok(result) => {
                        // Success logging moved to the run loop to avoid duplication
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request_id.unwrap_or(Value::Null),
                            result: Some(result),
                            error: None
                        }
                    }
                    Err(mcp_err) => {
                        let (code, message) = map_mcp_error_to_jsonrpc(mcp_err);
                        if code == error_codes::INTERNAL_ERROR || code < -32000 { // Log Internal and IMAP errors as ERROR
                             error!("Tool '{}' failed for id={:?}: [{}] {}", method_name, request_id, code, message);
                        } else { // Log InvalidParams, NotFound etc. as WARN
                             warn!("Tool '{}' failed for id={:?}: [{}] {}", method_name, request_id, code, message);
                        }
                        create_jsonrpc_error_response(request_id, code, message)
                    }
                }
            }
            None => {
                 let err_msg = format!("Method not found: {}", method_name);
                 warn!("MCP Method Not Found: {} (id: {:?})", method_name, request_id);
                 create_jsonrpc_error_response(request_id, error_codes::METHOD_NOT_FOUND, err_msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    // --- Mock Tools for Testing ---

    struct MockSuccessTool;
    #[async_trait]
    impl McpTool for MockSuccessTool {
        fn name(&self) -> &str { "test/success" }
        fn description(&self) -> &str { "A mock tool that always succeeds." }
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            // Optionally, check params if needed for specific tests
            Ok(json!({ "result": "success", "params_received": params }))
        }
    }

    struct MockFailureTool;
    #[async_trait]
    impl McpTool for MockFailureTool {
        fn name(&self) -> &str { "test/fail" }
        fn description(&self) -> &str { "A mock tool that always fails." }
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
            Err(McpPortError::ToolError("Mock tool failed as requested".to_string()))
        }
    }
    
    struct MockInvalidParamsTool;
    #[async_trait]
    impl McpTool for MockInvalidParamsTool {
        fn name(&self) -> &str { "test/invalidParams" }
        fn description(&self) -> &str { "A mock tool that expects specific params." }
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            #[allow(dead_code)] // Silence warning as we only check deserialization success
            #[derive(Deserialize)]
            struct ExpectedParams { required_field: String }
            match serde_json::from_value::<ExpectedParams>(params.clone()) {
                 Ok(_) => Ok(json!({ "result": "valid params received"})),
                 Err(_) => Err(McpPortError::InvalidParams(format!("Missing or invalid 'required_field' in params: {}", params))),
            }
        }
    }

    // --- Test Helper ---

    // Helper to run a single request/response cycle
    async fn run_single_request(
        tools: Vec<Arc<dyn McpTool>>,
        input_json_str: &str,
    ) -> Result<String, String> {
        // Create registry from provided mock tools for the test
        let mut tool_registry_map: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
        for tool in tools {
            tool_registry_map.insert(tool.name().to_string(), tool);
        }
        let tool_registry_arc = Arc::new(tool_registry_map);
        
        // Pass the Arc registry to the adapter
        let adapter = McpStdioAdapter::new(tool_registry_arc);

        // Use channels as a simpler way to simulate stdio for single request/response tests
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(1);
        let (stdout_tx, mut stdout_rx) = mpsc::channel::<String>(1);

        // Simulate writing to stdin
        stdin_tx.send(input_json_str.to_string()).await.map_err(|e| format!("Failed to send input: {}", e))?;

        // Process the line (simplified from the run loop)
        if let Some(line) = stdin_rx.recv().await {
             let response = adapter.process_line(&line).await;
             let response_str = serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))?;
             stdout_tx.send(response_str).await.map_err(|e| format!("Failed to send output: {}", e))?;
        } else {
             return Err("Stdin channel closed unexpectedly".to_string());
        }


        // Read from stdout
        stdout_rx.recv().await.ok_or_else(|| "Stdout channel closed unexpectedly".to_string())
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_success_request() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
        let input = r#"{"jsonrpc": "2.0", "id": 1, "method": "test/success", "params": {"arg1": "val1"}}"#;
        let expected_output = r#"{"jsonrpc":"2.0","id":1,"result":{"params_received":{"arg1":"val1"},"result":"success"}}"#;

        let result = run_single_request(tools, input).await;

        assert!(result.is_ok());
        // Compare parsed JSON Values for robustness against formatting differences
        let expected_val: Value = serde_json::from_str(expected_output).unwrap();
        let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(actual_val, expected_val);
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)]; // Only register success tool
        let input = r#"{"jsonrpc": "2.0", "id": 2, "method": "test/nonexistent", "params": {}}"#;
        // Expected error structure for METHOD_NOT_FOUND (-32601)
        let expected_output = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found: test/nonexistent"}}"#;

        let result = run_single_request(tools, input).await;

        assert!(result.is_ok());
        let expected_val: Value = serde_json::from_str(expected_output).unwrap();
        let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(actual_val, expected_val);
    }

    #[tokio::test]
    async fn test_tool_error() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockFailureTool)];
        let input = r#"{"jsonrpc": "2.0", "id": 3, "method": "test/fail", "params": null}"#;
        // Expected error structure for a ToolError mapped to IMAP_OPERATION_FAILED (-32010)
        let expected_output = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32010,"message":"Mock tool failed as requested"}}"#;

        let result = run_single_request(tools, input).await;

        assert!(result.is_ok());
        let expected_val: Value = serde_json::from_str(expected_output).unwrap();
        let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(actual_val, expected_val);
    }

    #[tokio::test]
    async fn test_parse_error() {
         let tools: Vec<Arc<dyn McpTool>> = vec![]; // No tools needed
         let input = r#"{"jsonrpc": "2.0", "id": 4, "method": "test/success"#; // Invalid JSON (missing closing })
         // Expected error for PARSE_ERROR (-32700)
         // Update the message pattern to match the actual error observed in test output
         let expected_pattern = r#""code":-32700,"message":"Parse error: EOF while parsing a string at line 1 column 51""#; 

         let result = run_single_request(tools, input).await;

         assert!(result.is_ok());
         let actual_output = result.unwrap();
         // Check if the relevant parts are present, allowing for minor variations
         assert!(actual_output.contains(r#""jsonrpc":"2.0""#));
         // For severely malformed JSON, serde might not be able to extract the ID.
         // Expect ID to be null in this case.
         assert!(actual_output.contains(r#""id":null"#), "Parse error response should contain 'id:null' for unrecoverable parse errors. Got: {}", actual_output);
         assert!(actual_output.contains(expected_pattern), "Actual output '{}' did not contain expected pattern '{}'", actual_output, expected_pattern);

         // Optional: stricter check if the exact message is reliable
         // let expected_val: Value = serde_json::from_str(expected_output).unwrap();
         // let actual_val: Value = serde_json::from_str(&actual_output).unwrap();
         // assert_eq!(actual_val, expected_val);
    }
    
    #[tokio::test]
    async fn test_invalid_params_error() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockInvalidParamsTool)];
        let input = r#"{"jsonrpc": "2.0", "id": 5, "method": "test/invalidParams", "params": {"wrong_field": 123}}"#;
        // Expected error for INVALID_PARAMS (-32602). Match the error message generated by the mock tool.
        let expected_output = r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32602,"message":"Missing or invalid 'required_field' in params: {\"wrong_field\":123}"}}"#;

        let result = run_single_request(tools, input).await;

        assert!(result.is_ok());
        let expected_val: Value = serde_json::from_str(expected_output).unwrap();
        let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(actual_val, expected_val);
    }
    
    #[tokio::test]
    async fn test_invalid_jsonrpc_version() {
         let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
         let input = r#"{"jsonrpc": "1.0", "id": 6, "method": "test/success", "params": {}}"#; // Invalid version
         // Expected error for INVALID_REQUEST (-32600)
         let expected_output = r#"{"jsonrpc":"2.0","id":6,"error":{"code":-32600,"message":"Invalid jsonrpc version, must be '2.0'"}}"#;

         let result = run_single_request(tools, input).await;

         assert!(result.is_ok());
         let expected_val: Value = serde_json::from_str(expected_output).unwrap();
         let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
         assert_eq!(actual_val, expected_val);
    }
    
    #[tokio::test]
    async fn test_request_without_id() {
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockSuccessTool)];
        // A request without an ID is a Notification. The server SHOULD NOT reply according to JSON-RPC 2.0 spec.
        // However, our current implementation *does* reply with id: null. Let's test that behavior.
        // If strict compliance is desired later, this test (and the implementation) should change.
        let input = r#"{"jsonrpc": "2.0", "method": "test/success", "params": {"data": "notify"}}"#;
        let expected_output = r#"{"jsonrpc":"2.0","id":null,"result":{"params_received":{"data":"notify"},"result":"success"}}"#;

        let result = run_single_request(tools, input).await;

        assert!(result.is_ok());
        let expected_val: Value = serde_json::from_str(expected_output).unwrap();
        let actual_val: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(actual_val, expected_val);
    }

}