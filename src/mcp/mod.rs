// src/mcp/mod.rs

pub mod types;
pub mod handler;
pub mod adapters;

// Re-export key types and traits for easier use
// pub use types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError}; // Unused
// pub use handler::McpHandler; // Unused 