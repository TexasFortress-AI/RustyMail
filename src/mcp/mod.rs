// src/mcp/mod.rs

// Core modules
pub mod types;
pub mod handler;
pub mod error_codes;
pub mod adapters;

// Re-export key types and traits for easier use
pub use types::{
    McpPortState,
    JsonRpcRequest,
    JsonRpcResponse,
    JsonRpcError,
    McpMessage,
    McpEvent,
    McpCommand,
    McpResult,
};

// Re-export handler trait
pub use handler::McpHandler;

// Re-export error codes
pub use error_codes::ErrorCode;

// Re-export adapter types
pub use adapters::{
    legacy::LegacyMcpAdapter,
    sdk::SdkMcpAdapter,
}; 