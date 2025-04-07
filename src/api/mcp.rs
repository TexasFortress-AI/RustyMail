use crate::prelude::*;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::imap::ImapClient; // Import ImapClient
use log::{debug, error, info, warn};
use crate::imap::types::{
    Folder as MappedFolder,
    Email as MappedEmail,
    AppendRequest as McpAppendRequest,
    AppendResponse as McpAppendResponse,
};
use crate::mcp_port::Session;
use crate::imap::client::ImapClient;
use crate::imap::error::ImapError;
use crate::imap::types::{FlagOperation, Flags, ModifyFlagsPayload, AppendEmailPayload, ExpungeResponse, MailboxInfo}; // Ensure all needed types are here
use crate::imap::types::SearchCriteria as ImapSearchCriteria;

// --- JSON-RPC Structures ---

#[derive(Deserialize, Debug)]
// Make public for tests
pub struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>, // Can be string, number, or null
    method: String,
    params: Option<Value>,
}

#[derive(Serialize, Debug)]
// Make public for tests
pub struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize, Debug)]
// Make public for tests
pub struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

// Specific error codes for JSON-RPC
// Make this module public for tests
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // -32000 to -32099: Server error
    pub const IMAP_CONNECTION_ERROR: i32 = -32000;
    pub const IMAP_AUTH_ERROR: i32 = -32001;
    pub const IMAP_FOLDER_NOT_FOUND: i32 = -32002;
    pub const IMAP_FOLDER_EXISTS: i32 = -32003;
    pub const IMAP_EMAIL_NOT_FOUND: i32 = -32004;
    pub const IMAP_OPERATION_FAILED: i32 = -32010; // Generic IMAP failure
}

// Map McpPortError to JsonRpcError parameters
fn map_mcp_error(e: McpPortError) -> (i32, String) {
    match e {
        McpPortError::InvalidParams(msg) => (error_codes::INVALID_PARAMS, msg),
        McpPortError::NotImplemented(msg) => (error_codes::METHOD_NOT_FOUND, msg), 
        McpPortError::ResourceError(msg) => (error_codes::INTERNAL_ERROR, msg),
        McpPortError::ToolError(msg) => {
            // Attempt to parse logical errors from the message string
            let lower_msg = msg.to_lowercase();
            if lower_msg.contains("already exists") {
                (error_codes::IMAP_FOLDER_EXISTS, msg)
            } else if lower_msg.contains("not found") { // Could be folder or email
                (error_codes::IMAP_FOLDER_NOT_FOUND, msg) // Use folder not found for simplicity
            } else if lower_msg.contains("authentication") || lower_msg.contains("login failed") {
                (error_codes::IMAP_AUTH_ERROR, msg)
            } else if lower_msg.contains("connection") {
                 (error_codes::IMAP_CONNECTION_ERROR, msg)
            // Add more specific string checks if needed for other ImapError variants
            } else {
                (error_codes::IMAP_OPERATION_FAILED, msg) // Generic operation failure
            }
        }
    }
}

// Helper to create an error response from McpPortError
// Make public for tests
pub fn create_mcp_error_response(id: Option<Value>, error: McpPortError) -> JsonRpcResponse {
    let (code, message) = map_mcp_error(error);
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.unwrap_or(Value::Null),
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: None,
        }),
    }
}

// Helper for standard JSON-RPC errors
// Make public for tests
pub fn create_jsonrpc_error_response(id: Option<Value>, code: i32, message: &str) -> JsonRpcResponse {
     JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.unwrap_or(Value::Null),
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
    }
}

// --- Concrete MCP Tools for IMAP ---

struct McpListFoldersTool {
    imap_client: Arc<ImapClient>,
}

#[async_trait]
impl McpTool for McpListFoldersTool {
    fn name(&self) -> &str { "imap/listFolders" }
    fn description(&self) -> &str { "Lists all IMAP folders." }

    async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
        let folders = self.imap_client.list_folders().await
            .map_err(|e| McpPortError::ToolError(format!("IMAP Error: {}", e)))?;
        Ok(serde_json::to_value(folders).unwrap_or(Value::Null))
    }
}

struct McpCreateFolderTool {
    imap_client: Arc<ImapClient>,
}

#[derive(Deserialize)]
struct CreateFolderParams {
    name: String,
}

#[async_trait]
impl McpTool for McpCreateFolderTool {
    fn name(&self) -> &str { "imap/createFolder" }
    fn description(&self) -> &str { "Creates a new IMAP folder." }

    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: CreateFolderParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?;
        if p.name.trim().is_empty() {
            return Err(McpPortError::InvalidParams("Folder name cannot be empty".into()));
        }
        self.imap_client.create_folder(&p.name).await
            // Map the specific ImapError if possible, otherwise ToolError
            .map_err(|e| McpPortError::ToolError(format!("{}", e)))?;
        Ok(serde_json::json!({ "message": "Folder created", "name": p.name }))
    }
}

struct McpDeleteFolderTool { imap_client: Arc<ImapClient> }
#[derive(Deserialize)] struct DeleteFolderParams { name: String }
#[async_trait]
impl McpTool for McpDeleteFolderTool {
    fn name(&self) -> &str { "imap/deleteFolder" }
    fn description(&self) -> &str { "Deletes an IMAP folder." }
    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: DeleteFolderParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?;
        self.imap_client.delete_folder(&p.name).await.map_err(|e| McpPortError::ToolError(format!("IMAP Error: {}", e)))?;
        Ok(serde_json::json!({ "message": "Folder deleted", "name": p.name }))
    }
}

struct McpRenameFolderTool { imap_client: Arc<ImapClient> }
#[derive(Deserialize)] struct RenameFolderParams { from: String, to: String }
#[async_trait]
impl McpTool for McpRenameFolderTool {
    fn name(&self) -> &str { "imap/renameFolder" }
    fn description(&self) -> &str { "Renames an IMAP folder." }
    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: RenameFolderParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?;
        if p.from.trim().is_empty() || p.to.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder names cannot be empty".into())); }
        self.imap_client.rename_folder(&p.from, &p.to).await.map_err(|e| McpPortError::ToolError(format!("IMAP Error: {}", e)))?;
        Ok(serde_json::json!({ "message": "Folder renamed", "old_name": p.from, "new_name": p.to }))
    }
}

struct McpSearchEmailsTool { imap_client: Arc<ImapClient> }
#[derive(Deserialize)] struct SearchEmailsParams { folder: String, criteria: String, value: Option<String> }
#[async_trait]
impl McpTool for McpSearchEmailsTool {
    fn name(&self) -> &str { "imap/searchEmails" }
    fn description(&self) -> &str { "Searches for emails in a folder based on criteria (All, Subject, From, To, Body, Since, Uid)." }
    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: SearchEmailsParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?;
        self.imap_client.select_folder(&p.folder).await.map_err(|e| McpPortError::ToolError(format!("IMAP Error selecting folder: {}", e)))?;
        let search_criteria = match p.criteria.to_lowercase().as_str() {
            "subject" => SearchCriteria::Subject(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for Subject".into()))?),
            "from" => SearchCriteria::From(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for From".into()))?),
            "to" => SearchCriteria::To(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for To".into()))?),
            "body" => SearchCriteria::Body(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for Body".into()))?),
            "since" => SearchCriteria::Since(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for Since".into()))?),
            "uid" => SearchCriteria::Uid(p.value.ok_or_else(|| McpPortError::InvalidParams("Missing value for Uid".into()))?),
            "all" => SearchCriteria::All,
            other => return Err(McpPortError::InvalidParams(format!("Unsupported criteria: {}", other)))
        };
        let uids = self.imap_client.search_emails(search_criteria).await.map_err(|e| McpPortError::ToolError(format!("IMAP search error: {}", e)))?;
        Ok(serde_json::to_value(uids).unwrap_or(Value::Null))
    }
}

struct McpFetchEmailsTool { imap_client: Arc<ImapClient> }
#[derive(Deserialize)] struct FetchEmailsParams { folder: Option<String>, uids: Vec<u32> }
#[async_trait]
impl McpTool for McpFetchEmailsTool {
    fn name(&self) -> &str { "imap/fetchEmails" }
    fn description(&self) -> &str { "Fetches email details by UID. Selects folder if provided." }
    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: FetchEmailsParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        // Select folder if specified, otherwise assume it's already selected
        if let Some(ref folder_name) = p.folder {
            self.imap_client.select_folder(folder_name).await.map_err(|e| McpPortError::ToolError(format!("IMAP Error selecting folder: {}", e)))?;
        }
        let fetch_body = false; // Default MCP fetch doesn't get body
        let emails = self.imap_client.fetch_emails(p.uids, fetch_body).await.map_err(|e| McpPortError::ToolError(format!("IMAP fetch error: {}", e)))?;
        // MCP likely expects a single email or needs adjustment for multiple
        emails.into_iter().next().ok_or_else(|| McpPortError::ResourceNotFound("Email not found".to_string()))
    }
}

struct McpMoveEmailTool { imap_client: Arc<ImapClient> }
#[derive(Deserialize)] struct MoveEmailParams { source_folder: Option<String>, uids: Vec<u32>, destination_folder: String }
#[async_trait]
impl McpTool for McpMoveEmailTool {
    fn name(&self) -> &str { "imap/moveEmail" }
    fn description(&self) -> &str { "Moves emails by UID to another folder. Selects source folder if provided." }
    async fn execute(&self, params: Value) -> Result<Value, McpPortError> {
        let p: MoveEmailParams = serde_json::from_value(params)
            .map_err(|e| McpPortError::InvalidParams(format!("Invalid params: {}", e)))?; 
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
         if p.destination_folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Destination folder cannot be empty".into())); }
        // Select source folder if specified
         if let Some(ref folder_name) = p.source_folder {
            self.imap_client.select_folder(folder_name).await.map_err(|e| McpPortError::ToolError(format!("IMAP Error selecting source folder: {}", e)))?;
        }
        self.imap_client.move_email(p.uids.clone(), &p.destination_folder).await.map_err(|e| McpPortError::ToolError(format!("IMAP move error: {}", e)))?;
        Ok(serde_json::json!({ "message": "Emails moved", "uids": p.uids, "destination": p.destination_folder }))
    }
}

// --- MCP Stdio Adapter ---

pub struct McpConfig {}

pub struct McpStdioAdapter {
    tool_registry: HashMap<String, Arc<dyn McpTool>>,
}

impl McpStdioAdapter {
    // Constructor now takes the ImapClient
    pub fn new(imap_client: Arc<ImapClient>) -> Self {
        let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();

        // Register IMAP tools
        let tools: Vec<Arc<dyn McpTool>> = vec![
            Arc::new(McpListFoldersTool { imap_client: imap_client.clone() }),
            Arc::new(McpCreateFolderTool { imap_client: imap_client.clone() }),
            Arc::new(McpDeleteFolderTool { imap_client: imap_client.clone() }),
            Arc::new(McpRenameFolderTool { imap_client: imap_client.clone() }),
            Arc::new(McpSearchEmailsTool { imap_client: imap_client.clone() }),
            Arc::new(McpFetchEmailsTool { imap_client: imap_client.clone() }),
            Arc::new(McpMoveEmailTool { imap_client: imap_client.clone() }),
        ];

        for tool in tools {
             tool_registry.insert(tool.name().to_string(), tool);
        }

        Self { tool_registry }
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut stdin = BufReader::new(io::stdin());
        let mut stdout = BufWriter::new(io::stdout());
        let mut line = String::new();

        info!("MCP Stdio Adapter Ready."); 

        loop {
            line.clear();
            match stdin.read_line(&mut line).await {
                Ok(0) => {
                    info!("MCP Stdio Adapter closing (EOF).");
                    break;
                } 
                Ok(_) => {
                    let trimmed_line = line.trim();
                    if trimmed_line.is_empty() { continue; }

                    debug!("Received raw MCP line: {}", trimmed_line);

                    let response: Option<JsonRpcResponse> = 
                        match serde_json::from_str::<JsonRpcRequest>(trimmed_line) {
                            Ok(req) => {
                                info!("Processing MCP Request id={:?}, method={}", req.id, req.method);
                                if req.jsonrpc != "2.0" {
                                    Some(create_jsonrpc_error_response(req.id, error_codes::INVALID_REQUEST, "Invalid jsonrpc version"))
                                } else {
                                    self.handle_request(req).await
                                }
                            },
                            Err(e) => {
                                // Attempt to parse just to get ID if possible, otherwise respond with null ID
                                let id = serde_json::from_str::<Value>(trimmed_line)
                                    .ok()
                                    .and_then(|v| v.get("id").cloned());
                                error!("MCP Parse error: {}", e);
                                Some(create_jsonrpc_error_response(id, error_codes::PARSE_ERROR, &format!("Parse error: {}", e)))
                            }
                        };

                    if let Some(resp_val) = response {
                        if let Ok(resp_str) = serde_json::to_string(&resp_val) {
                            info!("Sending MCP Response id={:?}", resp_val.id);
                            debug!("Raw MCP response: {}", resp_str);
                            stdout.write_all(resp_str.as_bytes()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                        } else {
                             error!("Failed to serialize MCP response: {:?}", resp_val);
                            // Maybe send a generic internal error? Difficult if serialization itself fails.
                        }
                    } // No response needed for notifications (if we handled them)
                }
                Err(e) => {
                    error!("Error reading stdin: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    // Handles valid JSON-RPC requests
    // Make public for tests if needed by run_adapter_test
    pub async fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let request_id = req.id.clone(); // Keep ID for response

        match self.tool_registry.get(&req.method) {
            Some(tool) => {
                let params = req.params.unwrap_or(Value::Null);
                match tool.execute(params).await {
                    Ok(result) => Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request_id.unwrap_or(Value::Null),
                        result: Some(result),
                        error: None,
                    }),
                    Err(e) => { // Use refined error mapping
                        warn!("MCP Tool execution error for method '{}': {}", req.method, e);
                        Some(create_mcp_error_response(request_id, e))
                    }
                }
            }
            None => {
                 warn!("MCP Method not found: {}", req.method);
                 Some(create_jsonrpc_error_response(request_id, error_codes::METHOD_NOT_FOUND, "Method not found"))
            }
        }
    }
} 