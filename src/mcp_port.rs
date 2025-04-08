use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use crate::imap::error::ImapError;
use crate::imap::client::ImapClient;
use std::sync::Arc;
use std::collections::HashMap;

// --- Bring in Tool Implementations from mcp_stdio.rs (or move them here) ---
// This assumes the tool structs (McpListFoldersTool, etc.) are accessible.
// If they remain private in mcp_stdio.rs, they need to be moved or made public.
// For now, let's assume they will be moved or made pub in mcp_stdio.rs.
use crate::api::mcp_stdio::{ 
    McpListFoldersTool,
    McpCreateFolderTool,
    McpDeleteFolderTool,
    McpRenameFolderTool,
    McpSelectFolderTool,
    McpSearchEmailsTool,
    McpFetchEmailsTool,
    McpMoveEmailTool,
    McpStoreFlagsTool,
    McpAppendEmailTool,
    McpExpungeFolderTool,
};

#[derive(Debug, Error)]
pub enum McpPortError {
    #[error("Tool execution failed: {0}")]
    ToolError(String),
    #[error("Resource access failed: {0}")]
    ResourceError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    #[error("Internal error: {message}")]
    InternalError { message: String },

    // --- IMAP Specific Errors ---
    #[error("IMAP Connection Error: {0}")]
    ImapConnectionError(String),
    #[error("IMAP Authentication Error: {0}")]
    ImapAuthenticationError(String),
    #[error("IMAP Folder Not Found: {0}")]
    ImapFolderNotFound(String),
    #[error("IMAP Folder Already Exists: {0}")]
    ImapFolderExists(String),
    #[error("IMAP Email Not Found: {0}")]
    ImapEmailNotFound(String),
    #[error("IMAP Operation Failed: {0}")]
    ImapOperationFailed(String),
    #[error("IMAP Invalid Criteria: {0}")]
    ImapInvalidCriteria(String),
    #[error("IMAP Requires Folder Selection: {0}")]
    ImapRequiresFolderSelection(String),
}

/// Defines a capability or action that can be executed via MCP.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// The unique name identifying this tool.
    fn name(&self) -> &str;
    
    /// A brief description of what the tool does.
    fn description(&self) -> &str;
    
    /// Executes the tool with the given parameters.
    async fn execute(&self, params: Value) -> Result<Value, McpPortError>;
}

/// Defines a resource whose state can be read via MCP.
#[async_trait]
pub trait McpResource: Send + Sync {
    /// The unique name identifying this resource.
    fn name(&self) -> &str;
    
    /// A brief description of the resource.
    fn description(&self) -> &str;

    /// Reads the current state of the resource.
    async fn read(&self) -> Result<Value, McpPortError>;
}

/// Creates and populates the MCP tool registry.
pub fn create_mcp_tool_registry(imap_client: Arc<ImapClient>) -> Arc<HashMap<String, Arc<dyn McpTool>>> {
    let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
    let tools: Vec<Arc<dyn McpTool>> = vec![
        // Instantiate tools using the provided ImapClient Arc
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
    for tool in tools {
        tool_registry.insert(tool.name().to_string(), tool);
    }
    Arc::new(tool_registry)
}

// Implement From<ImapError> for McpPortError
impl From<ImapError> for McpPortError {
    fn from(err: ImapError) -> Self {
        log::warn!("Converting IMAP Error to MCP Port Error: {:?}", err);
        match err {
            ImapError::Connection(m) => McpPortError::ImapConnectionError(m),
            ImapError::Tls(m) => McpPortError::ImapConnectionError(m), // Map TLS also to ConnectionError for MCP
            ImapError::Auth(m) => McpPortError::ImapAuthenticationError(m),
            ImapError::Parse(m) => McpPortError::ToolError(format!("IMAP Parse Error: {}", m)), // Maybe map to InvalidParams? Or ToolError?
            ImapError::BadResponse(m) => McpPortError::ToolError(format!("IMAP Bad Response: {}", m)),
            ImapError::Mailbox(m) => McpPortError::ImapFolderNotFound(m), // Assuming mailbox errors usually mean not found
            ImapError::Fetch(m) => McpPortError::ImapEmailNotFound(m), // Assuming fetch errors often relate to not found UIDs
            ImapError::Append(m) => McpPortError::ImapOperationFailed(m),
            ImapError::Operation(m) => McpPortError::ImapOperationFailed(m),
            ImapError::Command(m) => McpPortError::InvalidParams(m), // Command errors often due to bad params
            ImapError::Config(m) => McpPortError::InternalError { message: format!("IMAP Config Error: {}", m) },
            ImapError::Io(m) => McpPortError::ImapConnectionError(format!("IMAP IO Error: {}", m)),
            ImapError::Internal(m) => McpPortError::InternalError { message: format!("IMAP Internal Error: {}", m) },
            ImapError::EnvelopeNotFound => McpPortError::ImapEmailNotFound("Envelope data missing in fetch response".to_string()),
            
            // Add mappings for the new variants
            ImapError::FolderNotFound(m) => McpPortError::ImapFolderNotFound(m),
            ImapError::FolderExists(m) => McpPortError::ImapFolderExists(m),
            ImapError::RequiresFolderSelection(m) => McpPortError::ImapRequiresFolderSelection(m),
            ImapError::ConnectionError(m) => McpPortError::ImapConnectionError(m),
            ImapError::AuthenticationError(m) => McpPortError::ImapAuthenticationError(m),
            ImapError::EmailNotFound(uids) => McpPortError::ImapEmailNotFound(format!("UID(s) {:?} not found", uids)),
            ImapError::OperationFailed(m) => McpPortError::ImapOperationFailed(m),
            ImapError::InvalidCriteria(c) => McpPortError::ImapInvalidCriteria(format!("Invalid search criteria: {:?}", c)),
            ImapError::FolderNotSelected => McpPortError::ImapRequiresFolderSelection("Operation requires folder selection".to_string()),
            ImapError::ParseError(m) => McpPortError::ToolError(format!("IMAP Parse Error: {}", m)),
            // SessionError needs careful handling. Extract underlying info if possible.
            ImapError::SessionError(e) => {
                // Try to provide a more specific error based on the underlying async_imap::error::Error
                match e {
                    async_imap::error::Error::ConnectionLost => McpPortError::ImapConnectionError("Connection Lost".to_string()),
                    async_imap::error::Error::Parse(p_err) => McpPortError::ToolError(format!("IMAP Parse Error: {}", p_err)),
                    async_imap::error::Error::No(s) | async_imap::error::Error::Bad(s) => McpPortError::ImapOperationFailed(s),
                    async_imap::error::Error::Io(io_err) => McpPortError::ImapConnectionError(format!("IO Error: {}", io_err)),
                    // Handle other async_imap errors as specifically as possible, or fallback
                    _ => McpPortError::ImapOperationFailed(format!("Underlying IMAP Session Error: {}", e))
                }
            }
        }
    }
} 