pub mod client;
pub mod error;
pub mod session;
pub mod types;

#[cfg(test)]
mod client_test;

pub use client::ImapClient;
pub use error::ImapError;
pub use session::{ImapSession, TlsImapSession};
pub use types::{Email, Folder, SearchCriteria}; 