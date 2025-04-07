pub mod client;
pub mod types;
pub mod session;

#[cfg(all(test, feature = "integration_tests"))]
pub mod integration_test;

#[cfg(test)]
mod client_test;

mod error;

pub use client::ImapClient;
pub use error::ImapError;
pub use types::{Email, Folder, SearchCriteria}; 