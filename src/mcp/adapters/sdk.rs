// src/mcp/adapters/sdk.rs

use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex as TokioMutex;
use serde_json::{Value, json};
use log::{debug, error, info, warn};

// Declare mcp_port as a module
use crate::mcp_port; // Use the module

// Import our McpTool trait and the updated registry creator
use crate::mcp_port::{McpTool, create_mcp_tool_registry};
use crate::mcp::types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};
use crate::mcp::handler::{McpHandler, McpResult};
// Import session factory type correctly
use crate::imap::ImapSessionFactory;
// Import UnboundedSender correctly
use tokio::sync::mpsc::UnboundedSender;
use crate::imap::client::ImapClient;
use crate::imap::error::ImapError;
use crate::mcp_port::McpPortError; // Import McpPortError

// --- MCP SDK Integration --- 
// Stub out or remove rmcp imports if the crate is not used
/*
use rmcp::{
    model::{Request as SdkRequest, Response as SdkResponse, Error as SdkError, Id as SdkId, ErrorCode as SdkErrorCode, ResponsePayload as SdkResponsePayload, MethodRegistry, ParamsSpec, Param},
    server::{Server as SdkServer, ToolRegistry as SdkToolRegistry, Tool as SdkTool, Context as SdkContext, Error as SdkServerError, RpcToolError}, // Import RpcToolError
};
*/
// --- End MCP SDK Integration ---

// Remove McpParams import
// use rmcp::model::params::McpParams;

// --- Define SDK Context --- 
#[derive(Clone)] // rmcp might require Context to be Clone
pub struct RustyMailContext {
    // State specific to this request/connection
    pub port_state: McpPortState,
    // Factory to create IMAP sessions on demand for tools
    pub session_factory: ImapSessionFactory,
}

// Implement the rmcp Context trait (if required by rmcp - assuming basic marker trait or similar)
// impl SdkContext for RustyMailContext {}
// --- End SDK Context ---

// --- Tool Wrapper --- 
/// Wraps our McpTool to be compatible with rmcp::server::Tool
struct McpToolWrapper {
    mcp_tool: Arc<dyn McpTool + Send + Sync>,
}

// Stub out SdkTool and SdkServerError if rmcp is not used
// Remove #[async_trait] as it's applied to a struct impl, not a trait impl
// #[async_trait]
// impl SdkTool<RustyMailContext> for McpToolWrapper {
impl McpToolWrapper { // Temporary placeholder implementation
    fn name(&self) -> &str {
        self.mcp_tool.name()
    }

    /*
    fn params(&self) -> Option<ParamsSpec> {
        None
    }
    */

    async fn execute(&self, context: RustyMailContext, params: Option<Value>) -> Result<Option<Value>, JsonRpcError> { // Use JsonRpcError
        debug!("Executing MCP tool '{}' via SDK wrapper", self.mcp_tool.name());
        
        let mut state_guard = context.port_state.lock().await;
        
        // Remove McpParams conversion
        let params_value = params.unwrap_or(Value::Null);

        let imap_client = context.session_factory().await
            .map_err(|imap_err| {
                error!("Failed to create IMAP session for tool '{}': {}", self.name(), imap_err);
                // Convert ImapError to JsonRpcError
                JsonRpcError::from(imap_err) 
            })?;
        
        let session_arc = Arc::new(imap_client);

        match self.mcp_tool.execute(session_arc, &mut state_guard, params_value).await {
            Ok(result) => Ok(Some(result)),
            Err(jsonrpc_err) => {
                error!("MCP tool '{}' execution failed: {}", self.mcp_tool.name(), jsonrpc_err);
                Err(jsonrpc_err)
            }
        }
    }
}

// Remove or stub out rmcp-dependent helper functions
/*
fn map_mcp_code_to_sdk_code(mcp_code: i32) -> SdkErrorCode { ... }
fn sdk_id_to_jsonrpc_id(sdk_id: Option<SdkId>) -> Value { ... }
*/

/// Adapter implementing McpHandler using the official `rmcp` SDK.
#[derive(Debug)] // Added Debug
pub struct SdkMcpAdapter {
    // Stub out sdk_server if rmcp is not used
    // sdk_server: Arc<SdkServer<RustyMailContext>>,
    session_factory: ImapSessionFactory, 
}

impl SdkMcpAdapter {
    /// Creates a new SdkMcpAdapter.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing SdkMcpAdapter...");
        // Placeholder: Replace with actual SDK initialization
        // Example: let sdk_client = rmcp_sdk::init()?; 
        Ok(Self { session_factory: ImapSessionFactory::new()? })
    }

    // Helper to get an IMAP session
    async fn get_session(&self, context: &RustyMailContext) -> Result<ImapClient, McpPortError> {
        info!("SDK Adapter: Getting IMAP session via factory.");
        // Correctly call the factory
        (context.session_factory)()
            .await
            .map_err(McpPortError::ImapError)
    }
}

#[async_trait::async_trait]
impl McpHandler for SdkMcpAdapter {
    async fn handle_request(&self, request: JsonRpcRequest, port_state: McpPortState) -> McpResult {
        info!("SDK Adapter: Handling MCP request method: {}", request.method);
        
        // TODO: SDK interaction

        // Accessing session factory would require it to be part of SdkMcpAdapter state
        // or passed differently. For now, assume it's available if needed for context.
        // Example: Create context for potential SDK calls
        // let sdk_context = RustyMailContext {
        //    session_factory: self.session_factory.clone(), // If stored on self
        //    port_state: port_state.clone(),
        // };
        // let session = self.get_session(&sdk_context).await?;

        error!("SdkMcpAdapter handle_request is not implemented yet.");
        Err(McpPortError::MethodNotFound(request.method))
    }
}

// Remove commented out error mapping function
/*
fn map_mcp_error_to_sdk_error(mcp_err: mcp_port::McpError) -> RpcToolError {
    // ... logic ...
}
*/ 

// #[derive(Debug)] // Removed Debug derive
pub struct McpSdkState {
    pub session_factory: ImapSessionFactory, 
    pub sse_tx: Option<UnboundedSender<String>>,
    pub mcp_state: Arc<tokio::sync::Mutex<McpPortState>>, // Re-added Arc<Mutex<>>
} 