//! Handles MCP communication over stdin/stdout.

use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::imap::client::ImapClient;
use log::{debug, error, info, warn};
use crate::imap::types::{
    SearchCriteria
};
use crate::mcp_port::{McpPortError, McpTool};
use tokio::sync::Mutex as TokioMutex;
use crate::imap::types::{FlagOperation, Flags, AppendEmailPayload};

// Define state struct
#[derive(Debug, Clone, Default)] 
pub struct McpPortState {
    pub selected_folder: Option<String>,
}

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

// --- MCP Tool Trait and Macro ---

// Macro definition (Update struct visibility)
macro_rules! mcp_tool {
    // Variant with client
    ($tool_name:ident, $client_field:ident, $mcp_name:expr, $desc:expr, async fn execute($self:ident, $params_arg:ident : Value) -> Result<Value, McpPortError> $body:block) => {
        pub struct $tool_name {
            $client_field: Arc<ImapClient>,
        }

        impl $tool_name {
            #[allow(dead_code)]
            pub fn new($client_field: Arc<ImapClient>) -> Self {
                Self { $client_field }
            }
        }

        #[async_trait]
        impl McpTool for $tool_name {
            fn name(&$self) -> &str { $mcp_name }
            fn description(&$self) -> &str { $desc }
            
            // Correct signature: no state arg
            async fn execute(&$self, $params_arg: Value) -> Result<Value, McpPortError> {
                $body
            }
        }
    };
    // Variant without client
    ($tool_name:ident, $mcp_name:expr, $desc:expr, async fn execute($self:ident, $params_arg:ident : Value) -> Result<Value, McpPortError> $body:block) => {
        pub struct $tool_name;

        impl $tool_name {
            #[allow(dead_code)]
            pub fn new() -> Self {
                 Self
            }
        }
        
        #[async_trait]
        impl McpTool for $tool_name {
            fn name(&$self) -> &str { $mcp_name }
            fn description(&$self) -> &str { $desc }

            // Correct signature: no state arg
            async fn execute(&$self, $params_arg: Value) -> Result<Value, McpPortError> {
                 $body
            }
        }
    };
}

// Macro to deserialize parameters, return an error if deserialization fails
macro_rules! deserialize_params {
    ($params:expr, $param_type:ty) => {
        // Use map_err for concise error conversion
        serde_json::from_value::<$param_type>($params.clone())
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid parameters: {}", e)))
    };
}

// --- Tool Definitions using the Macro ---
// Update invocations to match the 2-argument execute signature
mcp_tool!(McpListFoldersTool, client, "imap/listFolders", "Lists all folders.", 
    async fn execute(self, _params: Value) -> Result<Value, McpPortError> {
        let folders = self.client.list_folders().await?;
        Ok(json!({ "folders": folders }))
    }
);

mcp_tool!(McpCreateFolderTool, client, "imap/createFolder", "Creates a new IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { name: String }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        // Prepend "INBOX." if it's not "INBOX"
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") {
            p.name.clone()
        } else {
            format!("INBOX.{}", p.name)
        };
        self.client.create_folder(&full_folder_name).await?;
        Ok(json!({ "message": "Folder created", "name": full_folder_name }))
    }
);

mcp_tool!(McpDeleteFolderTool, client, "imap/deleteFolder", "Deletes an IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { name: String }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        // Prepend "INBOX." if it's not "INBOX"
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") {
            p.name.clone()
        } else {
            format!("INBOX.{}", p.name)
        };
        self.client.delete_folder(&full_folder_name).await?;
        Ok(json!({ "message": "Folder deleted", "name": full_folder_name }))
    }
);

mcp_tool!(McpRenameFolderTool, client, "imap/renameFolder", "Renames an IMAP folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { from_name: String, to_name: String }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.from_name.trim().is_empty() || p.to_name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder names cannot be empty".into())); }
        // Prepend "INBOX." if necessary
        let full_from_name = if p.from_name.eq_ignore_ascii_case("INBOX") { p.from_name.clone() } else { format!("INBOX.{}", p.from_name) };
        let full_to_name = if p.to_name.eq_ignore_ascii_case("INBOX") { p.to_name.clone() } else { format!("INBOX.{}", p.to_name) };
        self.client.rename_folder(&full_from_name, &full_to_name).await?;
        Ok(json!({ "message": "Folder renamed", "from_name": full_from_name, "to_name": full_to_name }))
    }
);

// NOTE: McpSelectFolderTool now needs access to the mutable state to update selected_folder.
// This contradicts the change to McpTool::execute to remove the state parameter.
// SOLUTION: Let McpStdioAdapter handle the state update AFTER a successful execute call.
mcp_tool!(McpSelectFolderTool, client, "imap/selectFolder", "Selects a folder, making it active for subsequent commands.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { name: String }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        // Prepend "INBOX." if necessary
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        let mailbox_info = self.client.select_folder(&full_folder_name).await?;
        
        // Return the folder name along with mailbox info so the adapter can update state
        Ok(json!({ 
            "folder_name": full_folder_name,
            "mailbox_info": mailbox_info 
        }))
        // The state update logic is MOVED to the adapter's handle_request function.
    }
);

mcp_tool!(McpSearchEmailsTool, client, "imap/searchEmails", "Searches emails in the currently selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { criteria: SearchCriteria }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        // The actual search logic in ImapClient needs the selected folder context.
        // This assumes ImapClient::search_emails uses its internally tracked selected folder.
        let uids = self.client.search_emails(p.criteria).await?;
        Ok(json!({ "uids": uids }))
    }
);

mcp_tool!(McpFetchEmailsTool, client, "imap/fetchEmails", "Fetches emails by UID from the selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] struct Params { uids: Vec<u32>, fetch_body: Option<bool> }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        let fetch_body = p.fetch_body.unwrap_or(false);
        // Assumes ImapClient::fetch_emails uses its internally tracked selected folder.
        let emails = self.client.fetch_emails(p.uids, fetch_body).await?;
        Ok(serde_json::to_value(emails).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);

// NOTE: McpMoveEmailTool needs the source folder, which was previously obtained from state.
// SOLUTION: The McpStdioAdapter will inject the selected_folder from its state into the params BEFORE calling execute.
mcp_tool!(McpMoveEmailTool, client, "imap/moveEmails", "Moves emails by UID from the selected folder to a destination folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        #[derive(Deserialize)] 
        struct Params {
            uids: Vec<u32>, 
            destination_folder: String,
            source_folder: String, // Add source_folder, to be injected by adapter
        }
        // Remove ? operator
        let p: Params = deserialize_params!(params, Params)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        if p.destination_folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Destination folder cannot be empty".into())); }
        if p.source_folder.trim().is_empty() { return Err(McpPortError::ImapRequiresFolderSelection("Source folder missing in params".into())); } // Should be injected

        // Prepend "INBOX." if necessary for destination
        let full_destination_folder = if p.destination_folder.eq_ignore_ascii_case("INBOX") { p.destination_folder.clone() } else { format!("INBOX.{}", p.destination_folder) };

        self.client.move_email(&p.source_folder, p.uids.clone(), &full_destination_folder).await?;
        Ok(json!({ "message": "Emails moved", "uids": p.uids, "destination_folder": full_destination_folder, "source_folder": p.source_folder }))
    }
);

mcp_tool!(McpStoreFlagsTool, client, "imap/storeFlags", "Adds, removes, or sets flags for specified emails in the selected folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        // Deserialize directly into ModifyFlagsPayload (assuming it has operation: FlagOperation and flags: Flags)
        #[derive(Deserialize)] struct ModifyFlagsPayload { uids: Vec<u32>, operation: FlagOperation, flags: Flags }
        let p: ModifyFlagsPayload = deserialize_params!(params, ModifyFlagsPayload)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        if p.flags.items.is_empty() { return Err(McpPortError::InvalidParams("Flags list cannot be empty".into())); }
        
        // No need to convert operation, it's already FlagOperation
        // No need to manually construct flags, it's already Flags struct

        // Assumes ImapClient::store_flags uses its internally tracked selected folder.
        // Pass operation and flags directly
        self.client.store_flags(p.uids, p.operation, p.flags).await?;
        Ok(json!({ "message": "Flags stored successfully" }))
    }
);

mcp_tool!(McpAppendEmailTool, client, "imap/appendEmail", "Appends an email message to the specified folder.",
    async fn execute(self, params: Value) -> Result<Value, McpPortError> { 
        // Deserialize into a struct that matches the expected JSON params
        #[derive(Deserialize)] struct Params { folder: String, email: AppendEmailPayload } // email field is the payload
        let p: Params = deserialize_params!(params, Params)?;
        if p.folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        
        // Prepend "INBOX." if necessary
        let full_folder_name = if p.folder.eq_ignore_ascii_case("INBOX") { p.folder.clone() } else { format!("INBOX.{}", p.folder) };

        // Pass the deserialized AppendEmailPayload directly
        // No need for base64 decoding here, assume it's handled by AppendEmailPayload or ImapClient::append
        self.client.append(&full_folder_name, p.email).await?;
        Ok(json!({ "message": "Email appended", "folder": full_folder_name }))
    }
);

mcp_tool!(McpExpungeFolderTool, client, "imap/expungeFolder", "Permanently removes emails marked \\Deleted from the selected folder.",
    async fn execute(self, _params: Value) -> Result<Value, McpPortError> { 
        // Assumes ImapClient::expunge uses its internally tracked selected folder.
        self.client.expunge().await?;
        Ok(json!({ "message": "Expunge successful" }))
    }
);

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
    use crate::mcp_port::{McpTool, McpPortError};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::Arc;
    use async_trait::async_trait;
    use tokio::sync::Mutex as TokioMutex;

    // --- Mock Tools ---
    struct MockSuccessTool;
    #[async_trait]
    impl McpTool for MockSuccessTool {
        fn name(&self) -> &str { "test/success" }
        fn description(&self) -> &str { "A mock tool that always succeeds." }
        // Correct signature
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            Ok(json!({ "success": true, "params_received": params }))
        }
    }

    struct MockFailureTool;
    #[async_trait]
    impl McpTool for MockFailureTool {
        fn name(&self) -> &str { "test/fail" }
        fn description(&self) -> &str { "A mock tool that always fails." }
        // Correct signature
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
            Err(McpPortError::ToolError("Mock Failure".to_string()))
        }
    }

    struct MockInvalidParamsTool;
    #[async_trait]
    impl McpTool for MockInvalidParamsTool {
        fn name(&self) -> &str { "test/invalidParams" }
        fn description(&self) -> &str { "A mock tool that expects specific params." }
        // Correct signature
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
            #[derive(Deserialize)]
            struct ExpectedParams { required_field: String }
            // Use updated deserialize_params! macro
            let _p: ExpectedParams = deserialize_params!(params, ExpectedParams)?;
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
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockFailureTool)];
        let request = json!({ "jsonrpc": "2.0", "id": 3, "method": "test/fail" });
        let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["id"], 3);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::INTERNAL_ERROR); // ToolError maps to INTERNAL_ERROR
        assert_eq!(response["error"]["message"], "Tool execution failed: Mock Failure");
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
        let tools: Vec<Arc<dyn McpTool>> = vec![Arc::new(MockInvalidParamsTool)];
        let request = json!({ "jsonrpc": "2.0", "id": 5, "method": "test/invalidParams", "params": { "wrong_field": 123 } });
        let response_str = run_single_request(tools, &request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["id"], 5);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::INVALID_PARAMS);
        assert!(response["error"]["message"].as_str().unwrap().contains("Invalid parameters: missing field `required_field`"));
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
            fn name(&self) -> &str { "imap/selectFolder" }
            fn description(&self) -> &str { "Selects" }
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
            fn name(&self) -> &str { "imap/moveEmails" }
            fn description(&self) -> &str { "Moves" }
            async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
                #[derive(Deserialize, Debug)]
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