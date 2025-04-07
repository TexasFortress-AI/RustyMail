use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

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