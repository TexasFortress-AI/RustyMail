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
    params: Option<Value>,
) -> Result<Value, JsonRpcError> { // Use Value/JsonRpcError
    let folders = session.list_folders().await.map_err(|e| {
        // Create error with structured details including operation context
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("list_folders".to_string()));
        // Add params to the error data if available
        if let Some(p) = params.as_ref() {
            if let Some(data) = error.data.as_mut() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("params".to_string(), p.clone());
                }
            }
        }
        error
    })?;
    Ok(serde_json::to_value(folders).map_err(|e| JsonRpcError::internal_error(e.to_string()))?)
}

/// Tool for listing folders with hierarchical structure
pub async fn list_folders_hierarchical_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let folders = session.list_folders_hierarchical().await.map_err(|e| {
        // Create error with structured details including operation context
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("list_folders_hierarchical".to_string()));
        // Add params to the error data if available
        if let Some(p) = params.as_ref() {
            if let Some(data) = error.data.as_mut() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("params".to_string(), p.clone());
                }
            }
        }
        error
    })?;
    Ok(serde_json::to_value(folders).map_err(|e| JsonRpcError::internal_error(e.to_string()))?)
}

/// Tool for structured email search
pub async fn search_emails_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let search_criteria = if let Some(p) = params {
        // Try to deserialize search criteria from params
        serde_json::from_value::<crate::imap::types::SearchCriteria>(p.clone())
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid search criteria: {}", e)))?
    } else {
        // Default to All if no criteria provided
        crate::imap::types::SearchCriteria::All
    };

    let message_ids = session.search_emails_structured(&search_criteria).await.map_err(|e| {
        // Create error with structured details including operation context
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("search_emails".to_string()));
        // Add search criteria to the error data
        if let Some(data) = error.data.as_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("search_criteria".to_string(), serde_json::to_value(&search_criteria).unwrap_or_default());
            }
        }
        error
    })?;

    Ok(serde_json::to_value(message_ids).map_err(|e| JsonRpcError::internal_error(e.to_string()))?)
}

/// Tool for fetching emails with MIME part handling
pub async fn fetch_emails_with_mime_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    // Extract UIDs from params
    let uids = if let Some(p) = params {
        if let Some(uids_array) = p.get("uids") {
            if let Some(uids_vec) = uids_array.as_array() {
                uids_vec.iter()
                    .filter_map(|v| v.as_u64().map(|u| u as u32))
                    .collect::<Vec<u32>>()
            } else {
                return Err(JsonRpcError::invalid_params("uids must be an array of numbers"));
            }
        } else {
            return Err(JsonRpcError::invalid_params("uids parameter is required"));
        }
    } else {
        return Err(JsonRpcError::invalid_params("Parameters with uids are required"));
    };

    // Fetch emails with MIME parsing
    let emails = session.fetch_emails(&uids).await.map_err(|e| {
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("fetch_emails".to_string()));
        if let Some(data) = error.data.as_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("uids".to_string(), serde_json::to_value(&uids).unwrap_or_default());
            }
        }
        error
    })?;

    Ok(serde_json::to_value(emails).map_err(|e| JsonRpcError::internal_error(e.to_string()))?)
}

/// Tool for atomic move operations (single message)
pub async fn atomic_move_message_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    // Extract parameters
    let uid = params.get("uid")
        .and_then(|v| v.as_u64())
        .map(|u| u as u32)
        .ok_or_else(|| JsonRpcError::invalid_params("uid parameter is required as number"))?;

    let from_folder = params.get("from_folder")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("from_folder parameter is required as string"))?;

    let to_folder = params.get("to_folder")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("to_folder parameter is required as string"))?;

    // Perform atomic move operation
    session.move_email(uid, from_folder, to_folder).await.map_err(|e| {
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("atomic_move_message".to_string()));
        if let Some(data) = error.data.as_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("uid".to_string(), serde_json::to_value(&uid).unwrap_or_default());
                obj.insert("from_folder".to_string(), serde_json::Value::String(from_folder.to_string()));
                obj.insert("to_folder".to_string(), serde_json::Value::String(to_folder.to_string()));
            }
        }
        error
    })?;

    Ok(json!({
        "success": true,
        "message": format!("Successfully moved message UID {} from {} to {}", uid, from_folder, to_folder),
        "uid": uid,
        "from_folder": from_folder,
        "to_folder": to_folder
    }))
}

/// Tool for atomic batch move operations (multiple messages)
pub async fn atomic_batch_move_tool(
    session: Arc<dyn AsyncImapOps>,
    _state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Parameters are required"))?;

    // Extract UIDs
    let uids = if let Some(uids_array) = params.get("uids") {
        if let Some(uids_vec) = uids_array.as_array() {
            uids_vec.iter()
                .filter_map(|v| v.as_u64().map(|u| u as u32))
                .collect::<Vec<u32>>()
        } else {
            return Err(JsonRpcError::invalid_params("uids must be an array of numbers"));
        }
    } else {
        return Err(JsonRpcError::invalid_params("uids parameter is required"));
    };

    if uids.is_empty() {
        return Err(JsonRpcError::invalid_params("At least one UID must be provided"));
    }

    let from_folder = params.get("from_folder")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("from_folder parameter is required as string"))?;

    let to_folder = params.get("to_folder")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("to_folder parameter is required as string"))?;

    // Use the efficient batch move method
    session.move_messages(&uids, from_folder, to_folder).await.map_err(|e| {
        let mut error = crate::error::ErrorMapper::to_jsonrpc_error(&e, Some("atomic_batch_move".to_string()));
        if let Some(data) = error.data.as_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("uids".to_string(), serde_json::to_value(&uids).unwrap_or_default());
                obj.insert("from_folder".to_string(), serde_json::Value::String(from_folder.to_string()));
                obj.insert("to_folder".to_string(), serde_json::Value::String(to_folder.to_string()));
            }
        }
        error
    })?;

    Ok(json!({
        "success": true,
        "message": format!("Successfully moved {} messages from {} to {}",
            uids.len(), from_folder, to_folder),
        "moved_uids": uids,
        "from_folder": from_folder,
        "to_folder": to_folder
    }))
}

// Function to create and populate the registry
pub fn create_mcp_tool_registry() -> McpToolRegistry {
    let mut registry = McpToolRegistry::new();

    // Register tools using the DefaultMcpTool::new constructor
    registry.register("list_folders", DefaultMcpTool::new("list_folders", list_folders_tool));
    registry.register("list_folders_hierarchical", DefaultMcpTool::new("list_folders_hierarchical", list_folders_hierarchical_tool));
    registry.register("search_emails", DefaultMcpTool::new("search_emails", search_emails_tool));
    registry.register("fetch_emails_with_mime", DefaultMcpTool::new("fetch_emails_with_mime", fetch_emails_with_mime_tool));
    registry.register("atomic_move_message", DefaultMcpTool::new("atomic_move_message", atomic_move_message_tool));
    registry.register("atomic_batch_move", DefaultMcpTool::new("atomic_batch_move", atomic_batch_move_tool));
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