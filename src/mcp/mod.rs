// src/mcp/mod.rs

pub mod types;
pub mod handler;
pub mod adapters;

// Re-export key types/traits for easier use
pub use types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};
pub use handler::McpHandler; 