#![allow(unused_imports)]

// Public Interface for the IMAP module

pub mod atomic;
pub mod client;
pub mod error;
pub mod session;
pub mod types;

// --- Re-exports --- 
// Keep these minimal and focused on the public API

pub use client::ImapClient;
pub use error::ImapError;
pub use session::{AsyncImapOps, AsyncImapSessionWrapper};
pub use types::{
    Address, Email, Envelope, FlagOperation, Flags, Folder, MailboxInfo, SearchCriteria,
    // Re-export necessary payload types if they are part of the public API
    AppendEmailPayload, ModifyFlagsPayload, 
};

// --- Type Aliases (Consider if these are truly needed publicly) ---

// Remove unresolved AccountConfig import
// use crate::config::AccountConfig; // Needed for factory
use futures::future::BoxFuture; // Needed for factory
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::fmt;

// Import ImapClientFactory from session module
use crate::imap::session::ImapClientFactory;

// Result type for the factory
pub type ImapSessionFactoryResult = Result<ImapClient<AsyncImapSessionWrapper>, ImapError>;

// Add ImapSessionFactory as a type alias for ImapClientFactory
pub type ImapSessionFactory = ImapClientFactory;

// Cloneable wrapper for ImapSessionFactory
#[derive(Clone)]
pub struct CloneableImapSessionFactory {
    factory: Arc<ImapSessionFactory>,
}

impl CloneableImapSessionFactory {
    pub fn new(factory: ImapSessionFactory) -> Self {
        Self {
            factory: Arc::new(factory),
        }
    }

    pub async fn create_session(&self) -> ImapSessionFactoryResult {
        (self.factory)().await
    }
}

impl fmt::Debug for CloneableImapSessionFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloneableImapSessionFactory")
            .field("factory", &"<function>")
            .finish()
    }
}

// Previous commented-out definition for reference
// pub type ImapSessionFactory = Arc<dyn Fn(&AccountConfig) -> BoxFuture<ImapSessionFactoryResult> + Send + Sync>;

// --- Potentially Remove or Move Internal Re-exports ---
// These seem like internal details or duplicates from the top-level re-exports

// pub use client::{ImapClientBuilder}; // Builder might be internal or exposed differently
// pub use session::{TlsImapSession}; // Likely internal

// Remove duplicate imports if already covered by `pub use` or not needed
// use std::sync::Arc;
// use session::{TlsCompatibleStream}; // Likely internal

// Remove the test module re-export if it was temporary
// #[cfg(test)] // Only expose for tests if absolutely necessary
// pub mod client_test;