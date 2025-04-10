// src/mcp/handler.rs

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use serde_json::Value;
use crate::mcp::{McpPortState, JsonRpcRequest, JsonRpcResponse};
#[cfg(test)] // Only import mockall for tests
use mockall::automock;

/// Defines the asynchronous interface for handling MCP (JSON-RPC 2.0) requests.
///
/// This trait abstracts the core request processing logic. Implementations of this
/// trait are responsible for parsing the incoming JSON `Value` into a specific 
/// request structure (if applicable, based on the `method` field), executing the
/// requested operation (often by interacting with an IMAP client or other services),
/// and returning the result or error encapsulated in a JSON `Value` representing
/// a `JsonRpcResponse`.
///
/// The handler operates on a shared `McpPortState`, allowing it to access and 
/// potentially modify connection-specific context (like the selected IMAP folder).
///
/// Different transport adapters (like Stdio or SSE) will use an implementation
/// of this trait to process incoming messages.
#[cfg_attr(test, automock)] // Ensure attribute is present
#[async_trait]
pub trait McpHandler: Send + Sync {
    /// Handles a single incoming MCP (JSON-RPC) request represented as a `serde_json::Value`.
    ///
    /// # Arguments
    ///
    /// * `state` - An `Arc<Mutex<McpPortState>>` providing shared, mutable access 
    ///             to the state specific to the communication port (e.g., stdio session, 
    ///             SSE client connection). This allows the handler to use and update
    ///             context like the currently selected folder.
    /// * `json_req` - The raw JSON-RPC request object parsed as a `serde_json::Value`.
    ///                The handler implementation is responsible for further parsing 
    ///                this value based on the request's `method`.
    ///
    /// # Returns
    ///
    /// A `serde_json::Value` representing the serialized `JsonRpcResponse` (either
    /// success or error) to be sent back to the client.
    async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, json_req: Value) -> Value;
} 