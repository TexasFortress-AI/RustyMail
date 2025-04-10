// Publicly export key modules and types
pub mod client;
pub mod error;
pub mod session;
pub mod types;

// Make test module public for use in other tests
pub mod client_test;

// --- Add ImapSessionFactory definition --- 
use crate::imap::{
    client::ImapClient,
    error::ImapError,
};
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