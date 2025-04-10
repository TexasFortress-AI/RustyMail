// src/mcp/mod.rs

pub mod types;
pub mod handler;
pub mod error_codes;
pub mod adapters;

// Re-export key types and traits for easier use
pub use types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};
pub use handler::McpHandler;
// Also re-export the ErrorCode enum
pub use error_codes::ErrorCode; 