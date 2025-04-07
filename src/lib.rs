pub mod config;
pub mod imap;
pub mod transport;
pub mod mcp_port;

// Remove error and types from here, they belong inside imap
// pub mod error;
// pub mod types; 

// Re-export key types for convenience (optional, but common)
pub mod prelude {
    // Use correct paths based on module structure
    pub use crate::config::Settings;
    pub use crate::imap::{ImapClient, ImapSession}; // ImapSession is in imap::session, but might be re-exported in imap::mod.rs
    pub use crate::imap::error::ImapError; // Correct path
    pub use crate::imap::types::{Email, Folder, SearchCriteria}; // Correct path
    // pub use crate::transport::{/* Transport related types */}; // Keep commented if not ready
    pub use crate::mcp_port::{McpTool, McpResource, McpPortError};
}

#[cfg(test)]
mod transport_test; 