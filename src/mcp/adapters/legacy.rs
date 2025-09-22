// src/mcp/adapters/legacy.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::config::Settings;
use crate::imap::error::ImapError as ImapInternalError;
use crate::imap::session::AsyncImapOps;
use crate::prelude::CloneableImapSessionFactory;
use crate::mcp::handler::McpHandler;
use crate::mcp::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpPortState};
use crate::prelude::{McpTool};
use crate::prelude::*;

use serde_json::{json, Value};
use tokio::sync::Mutex as TokioMutex;

// pub use error_codes::*; // Unused

// --- Error Response Creation (Copied from mcp_stdio.rs, adjust if needed) ---
fn create_jsonrpc_error_response(id: Option<Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id,
        result: None,
        error: Some(JsonRpcError { code: code.into(), message, data: None }),
    }
}

/// Adapter implementing McpHandler using the original tool execution logic.
#[derive(Clone)]
pub struct LegacyMcpHandler {
    tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>,
    session_factory: CloneableImapSessionFactory,
}

impl LegacyMcpHandler {
    pub fn new(tool_registry: Arc<HashMap<String, Arc<dyn McpTool>>>, session_factory: CloneableImapSessionFactory) -> Self {
        info!("LegacyMcpHandler: Tool registry created with {} tools.", tool_registry.len());
        LegacyMcpHandler {
            tool_registry,
            session_factory,
        }
    }

    // Internal helper to process a request
    async fn process_request(
        &self,
        state: Arc<TokioMutex<McpPortState>>,
        req: JsonRpcRequest,
    ) -> JsonRpcResponse {
        let method = req.method.clone();
        let params_value = req.params.clone().unwrap_or(Value::Null);
        let request_id = req.id.clone();

        match self.tool_registry.get(&method) {
            Some(tool) => {
                debug!("Executing tool '{}' via LegacyMcpHandler", method);
                
                // Create IMAP session using the factory's method
                let session_result = self.session_factory.create_session().await;

                let session = match session_result {
                    Ok(client) => {
                        // Extract the Arc-wrapped session from ImapClient
                        client.session_arc() as Arc<dyn AsyncImapOps>
                    },
                    Err(imap_err) => {
                        error!("LegacyMcpHandler: Failed to create IMAP session for tool '{}': {:?}", method, imap_err);
                        let jsonrpc_err = JsonRpcError::from(imap_err);
                        return create_jsonrpc_error_response(request_id, jsonrpc_err.code as i32, jsonrpc_err.message);
                    }
                };

                // Acquire mutable lock and call execute with Value
                let mut state_guard = state.lock().await;
                let result = tool.execute(session.clone(), &mut state_guard, params_value).await;
                drop(state_guard); 

                match result {
                    Ok(result_value) => {
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request_id,
                            result: Some(result_value),
                            error: None,
                        }
                    }
                    Err(jsonrpc_err) => {
                        error!("Tool '{}' failed: {:?}", method, jsonrpc_err);
                        create_jsonrpc_error_response(request_id, jsonrpc_err.code as i32, jsonrpc_err.message)
                    }
                }
            }
            None => {
                warn!("Method not found: {}", method);
                let err = JsonRpcError::method_not_found();
                create_jsonrpc_error_response(request_id, err.code as i32, err.message)
            }
        }
    }
}

#[async_trait]
impl McpHandler for LegacyMcpHandler {
    async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, json_req: Value) -> Value {
        let request: JsonRpcRequest = match serde_json::from_value(json_req.clone()) {
            Ok(r) => r,
            Err(e) => {
                error!("LegacyAdapter: Failed to deserialize request: {}", e);
                return json!(JsonRpcResponse::parse_error());
            }
        };

        let response = self.process_request(state, request).await;
        
        match serde_json::to_value(response) {
            Ok(v) => v,
            Err(e) => {
                error!("LegacyAdapter: Failed to serialize response: {}", e);
                json!(JsonRpcResponse::error(
                    None,
                    JsonRpcError::server_error(
                        ErrorCode::InternalError as i64,
                        "Failed to serialize response".to_string()
                    )
                ))
            }

        }
    }
} 