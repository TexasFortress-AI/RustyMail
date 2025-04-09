// src/mcp/types.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// State relevant to an MCP connection/port.
#[derive(Debug, Clone, Default)] 
pub struct McpPortState {
    /// The currently selected IMAP folder for context-sensitive operations.
    pub selected_folder: Option<String>,
}

/// Represents a JSON-RPC 2.0 request.
#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// Represents a JSON-RPC 2.0 response.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Represents a JSON-RPC 2.0 error object.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
} 