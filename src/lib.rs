//! Library core for RustyMail.

// --- Modules ---
pub mod api;
pub mod config;
pub mod dashboard;
pub mod imap;
pub mod mcp;
pub mod transport;
pub mod mcp_port;

// Re-export key types for convenience
pub mod prelude {
    // Config
    pub use crate::config::Settings;

    // IMAP
    pub use crate::imap::{
        error::ImapError,
        session::AsyncImapSessionWrapper,
        types::{
            Address, AppendEmailPayload, Email, Envelope, FlagOperation, Flags,
            Folder, MailboxInfo, ModifyFlagsPayload, SearchCriteria, StoreOperation,
        },
    };

    // MCP / JSON-RPC
    pub use crate::mcp::{
        handler::McpHandler,
        types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpPortState},
    };

    // Common Libs
    pub use log::{debug, error, info, trace, warn};
    pub use std::sync::Arc;
    pub use thiserror::Error;
    pub use tokio::sync::Mutex as TokioMutex;
    pub use uuid::Uuid;
}

// Test modules
#[cfg(test)]
mod transport_test;
pub mod client_test;

// Main binary entry point
pub mod cli;

// Type alias for session factory
use crate::imap::{
    client::ImapClient,
    error::ImapError,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
};

pub type ImapSessionFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<ImapClient, ImapError>> + Send>>
        + Send
        + Sync,
>; 