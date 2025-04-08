pub mod config;
pub mod imap;
pub mod transport;
pub mod mcp_port;
pub mod api;

// Remove error and types from here, they belong inside imap
// pub mod error;
// pub mod types; 

// Re-export key types for convenience (optional, but common)
pub mod prelude {
    // Use correct paths based on module structure
    pub use crate::config::Settings;
    pub use crate::imap::{ImapClient, ImapSession, ImapError};
    pub use crate::imap::types::{Folder, Email, SearchCriteria};
    pub use imap_types::mailbox::Mailbox;
    pub use async_imap::types::NameAttribute;
    pub use imap_types::envelope::Envelope;
    pub use imap_types::envelope::Address;
    pub use imap_types::core::NString;
    pub use crate::api::mcp_stdio::error_codes as McpErrorCodes;
    pub use crate::mcp_port::McpPortError;
    pub use crate::mcp_port::McpTool;
}

#[cfg(test)]
mod transport_test; 