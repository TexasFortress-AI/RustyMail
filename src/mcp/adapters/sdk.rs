// src/mcp/adapters/sdk.rs

use async_trait::async_trait;
use std::sync::Arc;
use crate::prelude::CloneableImapSessionFactory;
use std::collections::HashMap;
use tokio::sync::Mutex as TokioMutex;
use serde_json::{Value, json};
use log::{debug, error, info};

// Import RMCP SDK types
use rmcp::{
    model::*,
    service::RequestContext,
    ServerHandler,
    RoleServer,
};

// Use our MCP types
use crate::mcp::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError, McpHandler};
use crate::mcp_port::{McpTool, create_mcp_tool_registry};

// Import session types
use tokio::sync::mpsc::UnboundedSender;
use crate::imap::error::ImapError;

// --- RustyMail Service Implementation ---
#[derive(Clone)]
pub struct RustyMailService {
    // State specific to this service
    pub port_state: Arc<TokioMutex<McpPortState>>,
    // Factory to create IMAP sessions on demand for tools
    pub session_factory: CloneableImapSessionFactory,
    // Tool registry containing all our MCP tools
    pub tool_registry: crate::mcp_port::McpToolRegistry,
}

impl RustyMailService {
    pub fn new(session_factory: CloneableImapSessionFactory) -> Self {
        let tool_registry = create_mcp_tool_registry();
        info!("RustyMailService: Tool registry created");

        Self {
            port_state: Arc::new(TokioMutex::new(McpPortState::default())),
            session_factory,
            tool_registry,
        }
    }

    // Wrapper method to call legacy MCP tools through the new SDK
    async fn execute_legacy_tool(&self, tool_name: String, params: Option<Value>) -> Result<CallToolResult, ErrorData> {
        debug!("Executing legacy tool '{}' via SDK", tool_name);

        let tool = self.tool_registry.get(&tool_name)
            .ok_or_else(|| ErrorData::new(
                -32601, // Method not found error code
                format!("Tool '{}' not found", tool_name),
                None
            ))?;

        // Create IMAP session
        let session_result = self.session_factory.create_session().await;
        let session = match session_result {
            Ok(client) => client.session_arc(),
            Err(imap_err) => {
                error!("Failed to create IMAP session for tool '{}': {:?}", tool_name, imap_err);
                return Err(ErrorData::new(
                    -32603, // Internal error code
                    format!("IMAP connection failed: {}", imap_err),
                    None
                ));
            }
        };

        // Execute the tool
        let mut state_guard = self.port_state.lock().await;
        let result = tool.execute(session, &mut state_guard, params.unwrap_or(Value::Null)).await;
        drop(state_guard);

        match result {
            Ok(value) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".to_string())
            )])),
            Err(err) => Err(ErrorData::new(
                err.code,
                err.message,
                err.data
            ))
        }
    }
}

// Implement ServerHandler for the service
impl ServerHandler for RustyMailService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "RustyMail MCP Server".to_string(),
            version: "0.1.0".to_string(),
            protocol_version: "0.1.0".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: None,
                }),
                ..Default::default()
            },
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.execute_legacy_tool(request.name.to_string(), request.arguments.and_then(|m| m.into_iter().next().map(|(_, v)| v))).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let items: Vec<Tool> = self.tool_registry.keys().map(|name| {
            Tool {
                name: name.clone().into(),
                description: Some(format!("IMAP tool: {}", name)),
                input_schema: Arc::new(serde_json::Map::new()),
                annotations: None,
            }
        }).collect();

        Ok(ListToolsResult {
            tools: items,
            next_cursor: None,
        })
    }
}

/// Adapter implementing McpHandler using the official RMCP SDK
pub struct SdkMcpAdapter {
    service: Arc<RustyMailService>,
}

impl SdkMcpAdapter {
    /// Creates a new SdkMcpAdapter.
    /// NOTE: Requires `CloneableImapSessionFactory` to be provided.
    pub fn new(session_factory: CloneableImapSessionFactory) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing SdkMcpAdapter...");
        let service = Arc::new(RustyMailService::new(session_factory));
        Ok(Self { service })
    }

    /// Temporary constructor that creates a new adapter with a placeholder factory
    /// until the proper factory can be injected
    pub fn new_placeholder() -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing SdkMcpAdapter with placeholder factory...");
        // This is a placeholder. In production, a real factory should be provided.
        let factory = CloneableImapSessionFactory::new(
            Box::new(|| Box::pin(async {
                Err(ImapError::Connection("Placeholder factory - not implemented".to_string()))
            }))
        );
        let service = Arc::new(RustyMailService::new(factory));
        Ok(Self { service })
    }
}

#[async_trait]
impl McpHandler for SdkMcpAdapter {
    /// Handles an MCP request by delegating to the appropriate tool
    async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, request: Value) -> Value {
        // Ensure input is a valid JsonRpcRequest structure before processing
        let rpc_request: JsonRpcRequest = match serde_json::from_value(request.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!("SDK Adapter: Received invalid JSON-RPC request object: {}", e);
                return serde_json::to_value(JsonRpcResponse::invalid_request()).unwrap_or(json!(null));
            }
        };

        info!("SDK Adapter: Handling MCP request method: {}", rpc_request.method);

        // Update the service's state with the provided state
        *self.service.port_state.lock().await = state.lock().await.clone();

        // Handle the request using our legacy tool wrapper
        let tool_request = CallToolRequestParam {
            name: rpc_request.method.clone().into(),
            arguments: rpc_request.params.and_then(|v| v.as_object().cloned()),
        };

        // Create a dummy context for the call
        // This is a workaround since we can't create RequestContext directly
        match self.service.execute_legacy_tool(
            rpc_request.method.clone(),
            rpc_request.params
        ).await {
            Ok(result) => {
                // Convert CallToolResult back to JsonRpcResponse
                let result_value = if !result.content.is_empty() {
                    json!({
                        "content": result.content.iter().map(|c| match c {
                            Content::Text(text_content) => json!({ "type": "text", "text": text_content.text }),
                            _ => json!(null),
                        }).collect::<Vec<_>>(),
                        "isError": result.is_error,
                    })
                } else {
                    json!(null)
                };

                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: rpc_request.id,
                    result: Some(result_value),
                    error: None,
                };
                serde_json::to_value(response).unwrap_or(json!(null))
            }
            Err(err) => {
                let error_response = JsonRpcResponse::error(
                    rpc_request.id,
                    JsonRpcError {
                        code: err.code as i32,
                        message: err.message.to_string(),
                        data: err.data,
                    }
                );
                serde_json::to_value(error_response).unwrap_or(json!(null))
            }
        }
    }
}

/// State specifically for the SdkMcpAdapter if needed (e.g., for SSE integration).
pub struct McpSdkState {
    pub session_factory: CloneableImapSessionFactory,
    pub sse_tx: Option<UnboundedSender<String>>,
    pub mcp_state: Arc<TokioMutex<McpPortState>>,
}