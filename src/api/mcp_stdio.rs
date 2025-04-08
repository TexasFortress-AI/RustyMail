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
        struct $struct_name {
            $client_field: Arc<ImapClient>,
        }
        impl $struct_name {
             fn new(client: Arc<ImapClient>) -> Self { Self { $client_field: client } }
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
    tool_registry: HashMap<String, Arc<dyn McpTool>>,
}

impl McpStdioAdapter {
    pub fn new(imap_client: Arc<ImapClient>) -> Self {
        let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
        let tools: Vec<Arc<dyn McpTool>> = vec![
            Arc::new(McpListFoldersTool::new(imap_client.clone())),
            Arc::new(McpCreateFolderTool::new(imap_client.clone())),
            Arc::new(McpDeleteFolderTool::new(imap_client.clone())),
            Arc::new(McpRenameFolderTool::new(imap_client.clone())),
            Arc::new(McpSelectFolderTool::new(imap_client.clone())),
            Arc::new(McpSearchEmailsTool::new(imap_client.clone())),
            Arc::new(McpFetchEmailsTool::new(imap_client.clone())),
            Arc::new(McpMoveEmailTool::new(imap_client.clone())),
            Arc::new(McpStoreFlagsTool::new(imap_client.clone())),
            Arc::new(McpAppendEmailTool::new(imap_client.clone())),
            Arc::new(McpExpungeFolderTool::new(imap_client.clone())),
        ];
        for tool in tools { tool_registry.insert(tool.name().to_string(), tool); }
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
                    },
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