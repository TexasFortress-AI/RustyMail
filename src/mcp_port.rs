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
        // Log the original IMAP error for debugging
        log::debug!("Converting ImapError to McpPortError: {:?}", err);

        match err {
            ImapError::Connection(m) => McpPortError::ImapConnectionError(m),
            ImapError::Tls(m) => McpPortError::ImapConnectionError(format!("TLS Error: {}", m)), // Map TLS to Connection for now
            ImapError::Auth(m) => McpPortError::ImapAuthenticationError(m),
            ImapError::Parse(m) => McpPortError::InternalError { message: format!("IMAP Parse Error: {}", m) }, // Treat parse as Internal
            ImapError::BadResponse(m) => McpPortError::InternalError { message: format!("IMAP Bad Response: {}", m) }, // Treat bad response as Internal
            ImapError::Mailbox(m) => McpPortError::ImapOperationFailed(format!("Mailbox Error: {}", m)), // Generic operation failure
            ImapError::Fetch(m) => McpPortError::ImapOperationFailed(format!("Fetch Error: {}", m)), // Generic operation failure
            ImapError::Append(m) => McpPortError::ImapOperationFailed(format!("Append Error: {}", m)), // Generic operation failure
            ImapError::Operation(m) => McpPortError::ImapOperationFailed(m),
            ImapError::Command(m) => McpPortError::ImapOperationFailed(format!("Command Error: {}", m)), // Generic operation failure
            ImapError::Config(m) => McpPortError::InternalError { message: format!("IMAP Config Error: {}", m) }, // Treat config as Internal
            ImapError::Io(m) => McpPortError::ImapConnectionError(format!("IO Error: {}", m)), // Map IO to Connection
            ImapError::Internal(m) => McpPortError::InternalError { message: m },
            ImapError::EnvelopeNotFound => McpPortError::ImapEmailNotFound("Envelope not found".to_string()),
            ImapError::FolderNotFound(m) => McpPortError::ImapFolderNotFound(m),
            ImapError::FolderExists(m) => McpPortError::ImapFolderExists(m),
            ImapError::RequiresFolderSelection(m) => McpPortError::ImapRequiresFolderSelection(m),
            // Add specific cases for other ImapError variants if they exist and need distinct McpPortError types
            // Example: If ImapError had a variant like `InvalidSearch`, map it:
            // ImapError::InvalidSearch(m) => McpPortError::ImapInvalidCriteria(m),
        }
    }
} 