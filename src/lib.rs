pub mod config;
pub mod imap;
pub mod transport;
pub mod mcp_port;

// Remove error and types from here, they belong inside imap
// pub mod error;
// pub mod types; 

// Re-export key types for convenience (optional, but common)
pub mod prelude {
    // Example re-exports, adjust as needed
    pub use crate::config::Settings;
    pub use crate::imap::{ImapClient, ImapSession};
    pub use crate::error::ImapError;
    pub use crate::types::{Email, Folder, SearchCriteria};
    pub use crate::transport::{/* Transport related types */};
    pub use crate::mcp_port::{McpTool, McpResource, McpPortError};
}

#[cfg(test)]
mod transport_test; 