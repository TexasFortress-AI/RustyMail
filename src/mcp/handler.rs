// src/mcp/handler.rs

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use serde_json::Value;
use crate::mcp::types::{McpPortState, JsonRpcRequest, JsonRpcResponse};
#[cfg(test)] // Only import mockall for tests
use mockall::automock;

/// Defines the interface for handling MCP (JSON-RPC) requests.
/// This trait allows different adapters (like Stdio, SSE, or an SDK-based one)
/// to process requests using a common underlying logic or SDK.
#[cfg_attr(test, automock)] // Ensure attribute is present
#[async_trait]
pub trait McpHandler: Send + Sync {
    /// Handles a single JSON-RPC request.
    ///
    /// Takes the shared, mutable state specific to the communication port 
    /// (e.g., stdio session, SSE client connection) and the request object.
    /// Returns a JSON-RPC response.
    async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, json_req: Value) -> Value;
} 