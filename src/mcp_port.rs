use std::sync::Arc;
use serde_json::{Value, json};
use tokio::sync::Mutex as TokioMutex;
use crate::mcp::{types::{JsonRpcError, McpPortState}};
use crate::imap::client::ImapClient;
use crate::imap::session::AsyncImapSessionWrapper;
use base64::{engine::general_purpose, Engine as _};
use log::{info, warn, error, debug};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use futures_util::future::BoxFuture;
use crate::prelude::AsyncImapOps;

// Define the signature for an MCP tool function
// The function receives the IMAP session, MCP state, and optional parameters.
// It returns a BoxFuture which resolves to a Result<McpResult, McpError>.
// Commenting out McpResult, McpError as they are unresolved
type McpToolFn = Box<
    dyn Fn(
        Arc<dyn AsyncImapOps>,
        Arc<TokioMutex<McpPortState>>,
        Option<Value>,
    ) -> BoxFuture<'static, Result<Value, JsonRpcError>> // Use Value/JsonRpcError for now
    + Send
    + Sync,
>;

/// Trait that defines the interface for a tool that can be executed via MCP.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Executes the tool with the given session, state, and parameters.
    async fn execute(
        &self,
        session: Arc<dyn AsyncImapOps>,
        state: &mut McpPortState,
        params: Value,
    ) -> Result<Value, JsonRpcError>;
    
    /// Returns the name of the tool.
    fn name(&self) -> &str;
}

/// Default implementation of McpTool
pub struct DefaultMcpTool {
    name: String,
    func: McpToolFn,
}

// Commenting out McpResult, McpError as they are unresolved
impl DefaultMcpTool {
    /// Creates a new DefaultMcpTool.
    pub fn new<
        F: Fn(
                Arc<dyn AsyncImapOps>,
                Arc<TokioMutex<McpPortState>>,
                Option<Value>,
            ) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Value, JsonRpcError>> + Send + 'static, // Use Value/JsonRpcError
    >(
        name: &str,
        f: F,
    ) -> Self {
        DefaultMcpTool {
            name: name.to_string(),
            func: Box::new(move |session, state, params| Box::pin(f(session, state, params))),
        }
    }
    
    /// Executes the tool.
    pub async fn execute_internal(
        &self,
        session: Arc<dyn AsyncImapOps>,
        state: Arc<TokioMutex<McpPortState>>,
        params: Option<Value>,
    ) -> Result<Value, JsonRpcError> { // Use Value/JsonRpcError
        (self.func)(session, state, params).await
    }
}

#[async_trait]
impl McpTool for DefaultMcpTool {
    async fn execute(
        &self,
        session: Arc<dyn AsyncImapOps>,
        state: &mut McpPortState,
        params: Value,
    ) -> Result<Value, JsonRpcError> {
        // Wrap the state in an Arc<TokioMutex<>> for the internal implementation
        let state_arc = Arc::new(TokioMutex::new(state.clone()));
        let result = self.execute_internal(session, state_arc.clone(), Some(params)).await;
        
        // Update the original state with any changes from the mutex
        if let Ok(mutex_state) = state_arc.try_lock() {
            *state = mutex_state.clone();
        }
        
        result
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

/// Registry for MCP tools.
#[derive(Default)]
pub struct McpToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register<T: McpTool + 'static>(&mut self, name: &str, tool: T) {
        self.tools.insert(name.to_string(), Box::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn McpTool>> {
        self.tools.get(name)
    }
}

// --- Tool Implementations ---

// Example Tool: List Folders
// Commenting out McpResult, McpError
pub async fn list_folders_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    _params: Option<Value>,
) -> Result<Value, JsonRpcError> { // Use Value/JsonRpcError
    let folders = session.list_folders().await.map_err(JsonRpcError::from)?;
    Ok(serde_json::to_value(folders).map_err(|e| JsonRpcError::internal_error(e.to_string()))?)
}

// ... Implement other tools similarly, receiving state ...

// Function to create and populate the registry
pub fn create_mcp_tool_registry() -> McpToolRegistry {
    let mut registry = McpToolRegistry::new();

    // Register tools using the DefaultMcpTool::new constructor
    registry.register("list_folders", DefaultMcpTool::new("list_folders", list_folders_tool));
    // ... register other tools like create_folder_tool, delete_folder_tool etc.
    // These tools will need to be defined similar to list_folders_tool

    registry
}

/// Defines a resource whose state can be read via MCP.
#[async_trait]
pub trait McpResource: Send + Sync {
    /// The unique name identifying this resource.
    fn name(&self) -> &str;
    
    /// A brief description of the resource.
    fn description(&self) -> &str;

    /// Reads the current state of the resource.
    async fn read(&self) -> Result<Value, JsonRpcError>;

    // Add more resource methods as needed
} 