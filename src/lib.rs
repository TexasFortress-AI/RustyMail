//! Library core for RustyMail.

// --- Modules ---
pub mod api;
pub mod config;
pub mod dashboard;
pub mod imap;
pub mod mcp;
pub mod mcp_port;
// pub mod transport; // Commented out as it might be unused or integrated elsewhere

// Re-export key types for convenience (optional, but common)
// CONSOLIDATED PRELUDE
pub mod prelude {
    // Config
    pub use crate::config::Settings;

    // IMAP
    pub use crate::imap::error::ImapError;
    pub use crate::imap::types::{Email, Folder, SearchCriteria, Flags, FlagOperation, MailboxInfo, UidSet, Envelope, Address, ModifyFlagsPayload, AppendEmailPayload, StoreOperation};
    pub use crate::imap::session::{AsyncImapSessionWrapper, StoreOperation};

    // MCP / JSON-RPC
    pub use crate::mcp::handler::McpHandler;
    pub use crate::mcp_port::McpPortError;
    pub use crate::mcp::types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};
    // pub use crate::mcp_port::McpTool; // McpTool might be internal to mcp_port now

    // Common Libs
    pub use log::{debug, error, info, trace, warn};
    pub use thiserror::Error;
    pub use uuid::Uuid;
    pub use std::sync::Arc;
    pub use tokio::sync::Mutex as TokioMutex;
}

#[cfg(test)]
mod transport_test; 

// --- Main Binary Entry Point Logic ---
pub mod cli;

// REMOVED Duplicate module declarations
// REMOVED Duplicate prelude definitions 

// Make test module public for use in other tests
pub mod client_test;

// --- Add ImapSessionFactory definition --- 
use crate::imap::{
    client::ImapClient,
    error::ImapError,
};
// Import needed types for factory
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
};

/// Type alias for a factory function that creates new IMAP client sessions.
///
/// This factory is expected to handle connection and login, returning a ready-to-use `ImapClient`.
pub type ImapSessionFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<ImapClient, ImapError>> + Send>>
        + Send
        + Sync,
>;

// pub use client::ImapClient; // Unused
// pub use error::ImapError; // Unused
// pub use session::{ImapSession, TlsImapSession}; // Unused
// pub use types::{Email, Folder, SearchCriteria}; // Unused 