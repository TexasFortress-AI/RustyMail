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

// --- Error Mapping (Updated to use McpPortError from mcp_port.rs) ---

// Create response for non-IMAP errors (like InvalidParams)
fn create_mcp_error_response_direct(id: Option<Value>, err: McpPortError) -> JsonRpcResponse {
     let (code, message) = match err {
        McpPortError::InvalidParams(m) => (error_codes::INVALID_PARAMS, m),
        McpPortError::ToolError(m) => (error_codes::IMAP_OPERATION_FAILED, m), // Default code for ToolError
        McpPortError::ResourceError(m) => (error_codes::INTERNAL_ERROR, m), // Map ResourceError if used
        McpPortError::NotImplemented(m) => (error_codes::METHOD_NOT_FOUND, m), // Map NotImplemented if used
        // Add the InternalError case
        McpPortError::InternalError { message } => (error_codes::INTERNAL_ERROR, message),
     };
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

mcp_tool!(McpExpungeFolderTool, client, "imap/expungeFolder", "Permanently removes emails marked \\Deleted from the selected folder.",
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
                        info!("Sending MCP Response id={:?}", response.id);
                        debug!("Raw MCP response: {}", resp_str);
                        stdout.write_all(resp_str.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    } else {
                        error!("Failed to serialize MCP response: {:?}", response);
                        let err_resp = create_mcp_error_response_direct(
                            Some(response.id), 
                            McpPortError::ToolError("Failed to serialize response".to_string()),
                        );
                        if let Ok(err_resp_str) = serde_json::to_string(&err_resp) {
                           stdout.write_all(err_resp_str.as_bytes()).await?;
                           stdout.write_all(b"\n").await?;
                           stdout.flush().await?;
                        }
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
                    return create_mcp_error_response_direct(req.id, McpPortError::InvalidParams("Invalid jsonrpc version".to_string()));
                } 
                self.handle_request(req).await
            }
            Err(e) => {
                 let id = serde_json::from_str::<Value>(line).ok().and_then(|v| v.get("id").cloned());
                 error!("MCP Parse error: {}", e);
                 create_mcp_error_response_direct(id, McpPortError::InvalidParams(format!("Parse error: {}", e)))
            }
        }
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let request_id = req.id.clone();
        match self.tool_registry.get(&req.method) {
            Some(tool) => {
                let params = req.params.unwrap_or(Value::Null);
                debug!("Executing tool '{}' with params: {}", req.method, params);
                match tool.execute(params).await {
                    Ok(result) => {
                        info!("Tool '{}' executed successfully for id={:?}", req.method, request_id);
                        JsonRpcResponse { jsonrpc: "2.0".to_string(), id: request_id.unwrap_or(Value::Null),
                                          result: Some(result), error: None }
                    },
                    Err(e @ McpPortError::InvalidParams(_)) => {
                        warn!("Tool '{}' failed: {}", req.method, e);
                        create_mcp_error_response_direct(request_id, e)
                    }
                     Err(e @ McpPortError::ToolError(_)) => { 
                         warn!("Tool '{}' failed: {}", req.method, e);
                         create_mcp_error_response_direct(request_id, e)
                    }
                    Err(e) => { 
                        error!("Tool '{}' failed unexpectedly: {}", req.method, e);
                        create_mcp_error_response_direct(request_id, e)
                    }
                }
            }
            None => {
                 warn!("MCP Method not found: {}", req.method);
                 create_mcp_error_response_direct(request_id, McpPortError::NotImplemented("Method not found".to_string()))
            }
        }
    }
} 