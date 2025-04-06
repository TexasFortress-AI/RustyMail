pub mod imap;
pub mod models;
pub mod error;
pub mod utils;

// Re-export key types for consumers
pub use imap::*;
pub use models::*;
pub use error::*;
pub use utils::*;
