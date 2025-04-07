use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use crate::imap::error::ImapError;

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

// Implement From<ImapError> for McpPortError
impl From<ImapError> for McpPortError {
    fn from(err: ImapError) -> Self {
        log::error!("IMAP operation failed: {:?}", err);
        // Map ImapError to a generic McpPortError::InternalError
        // You might refine this mapping later for more specific error codes if needed
        McpPortError::InternalError { 
            message: format!("Internal IMAP error: {}", err) 
        }
    }
} 