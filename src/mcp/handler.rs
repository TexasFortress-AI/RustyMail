// src/mcp/handler.rs

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use crate::mcp::types::{McpPortState, JsonRpcRequest, JsonRpcResponse};

/// Defines the core interface for handling MCP requests.
/// Adapters implement this trait to provide specific MCP handling logic
/// (e.g., using the legacy code or the official SDK).
#[async_trait]
pub trait McpHandler: Send + Sync {
    /// Handles a deserialized JSON-RPC request within a given state context.
    ///
    /// # Arguments
    ///
    /// * `state` - An Arc-Mutex-protected `McpPortState` representing the current connection state.
    /// * `request` - The deserialized `JsonRpcRequest`.
    ///
    /// # Returns
    ///
    /// A `JsonRpcResponse` representing the outcome of the request processing.
    async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, request: JsonRpcRequest) -> JsonRpcResponse;
} 