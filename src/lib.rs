//! Library core for RustyMail.

// --- Modules ---
pub mod api;
pub mod config;
pub mod dashboard;
pub mod imap;
pub mod mcp;
pub mod transport;
pub mod mcp_port;
pub mod session_manager;

// Re-export key types for convenience
pub mod prelude {
    // Config
    pub use crate::config::Settings;

    // IMAP
    pub use crate::imap::{
        client::ImapClient,
        error::ImapError,
        session::{AsyncImapOps, AsyncImapSessionWrapper, TlsImapSession, ImapClientFactory},
        types::{
            Address, AppendEmailPayload, Email, Envelope, FlagOperation, Flags,
            Folder, MailboxInfo, ModifyFlagsPayload, SearchCriteria,
        },
        ImapSessionFactory, CloneableImapSessionFactory,
    };

    // MCP / JSON-RPC
    pub use crate::mcp::{
        handler::McpHandler,
        types::{
            JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpPortState
        },
        error_codes::ErrorCode,
    };

    // MCP Tools
    pub use crate::mcp_port::{McpTool, DefaultMcpTool};

    // Common Libs
    pub use log::{debug, error, info, trace, warn};
    pub use std::sync::Arc;
    pub use thiserror::Error;
    pub use tokio::sync::Mutex as TokioMutex;
    pub use uuid::Uuid;

    // Session management
    pub use crate::session_manager::{
        SessionManager, 
        SessionManagerTrait, 
        SessionError, 
        SessionResult
    };
    
    #[cfg(test)]
    pub use crate::session_manager::mock::MockSessionManager;
    #[cfg(test)]
    pub use crate::session_manager::mock::MockSessionManagerTrait;
}

// Test modules
#[cfg(test)]
mod transport_test;
#[cfg(test)]
pub mod client_test;

// Main binary entry point
pub mod cli;

// Type alias for session factory - COMMENTED OUT
// pub use crate::imap::ImapSessionFactory;
pub use crate::mcp_port::McpToolRegistry;

// REMOVE FROM HERE DOWN
// pub mod api;
// pub mod config;
// pub mod imap;
// pub mod mcp;
// pub mod mcp_port;
// 
// // Re-exports for easier access
// // pub use config::Settings; // Duplicate removed
// // pub use imap::client::ImapClient; // Duplicate removed
// // pub use imap::error::ImapError; // Duplicate removed
// // pub use imap::session::AsyncImapOps; // Duplicate removed
// // pub use imap::session::ImapClientFactory; // Duplicate removed
// // pub use imap::session::{TlsImapSession, AsyncImapSessionWrapper}; // Duplicate removed
// // pub use mcp::handler::{McpHandler, JsonRpcHandler, MockMcpHandler}; // Duplicate removed
// // pub use mcp::types::{McpCommand, McpEvent, McpMessage, McpResult}; // Duplicate removed
// // use crate::imap::session::DEFAULT_MAILBOX_DELIMITER; // Duplicate removed
// 
// // Potential entry points or utilities can be defined here
// // For example, a function to initialize the whole system
// // pub async fn initialize_system(settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
// //     // ... initialization logic ...
// //     Ok(())
// // } 