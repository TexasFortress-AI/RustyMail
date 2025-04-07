use thiserror::Error;
// Remove FlagError import as it doesn't seem to exist
// use imap_types::flag::FlagError;
// Remove unused Infallible
// use std::convert::Infallible;

#[derive(Error, Debug)]
pub enum ImapError {
    #[error("Connection Error: {0}")]
    Connection(String),
    #[error("TLS Error: {0}")]
    Tls(String),
    #[error("Authentication Error: {0}")]
    Auth(String),
    #[error("Parse Error: {0}")]
    Parse(String),
    #[error("Bad Server Response: {0}")]
    BadResponse(String),
    #[error("Mailbox Operation Error: {0}")]
    Mailbox(String),
    #[error("Fetch Error: {0}")]
    Fetch(String),
    #[error("Append Error: {0}")]
    Append(String),
    #[error("Operation Error: {0}")]
    Operation(String),
    #[error("Command Error: {0}")]
    Command(String),
    #[error("Configuration Error: {0}")]
    Config(String),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    // Add other specific errors as needed from async-imap or imap-types
}

// Optional: Add From implementations if needed for converting errors
// from underlying libraries like tokio_rustls::rustls::Error
impl From<async_imap::error::Error> for ImapError {
    fn from(err: async_imap::error::Error) -> Self {
        // Map specific async_imap errors to our variants
        // Keep only variants that exist based on previous check errors
        match err {
            // Remove Connection, Tls, BadResponse as they caused errors
            // async_imap::error::Error::Connection(e) => ImapError::Connection(e.to_string()),
            // async_imap::error::Error::Tls(e) => ImapError::Tls(e.to_string()),
            async_imap::error::Error::Parse(e) => ImapError::Parse(e.to_string()),
            // async_imap::error::Error::BadResponse(resp) => ImapError::BadResponse(String::from_utf8_lossy(&resp).into_owned()),
            async_imap::error::Error::Append => ImapError::Append("Failed to append message".to_string()),
            // Add mappings for Auth, etc. if async_imap provides them, otherwise use Operation
            // For example (check async-imap docs for actual variants):
            // async_imap::error::Error::Auth(e) => ImapError::Auth(e.to_string()),
            other => ImapError::Operation(other.to_string()), // Default catch-all
        }
    }
}

// Remove From implementation for FlagError
// impl From<FlagError<'_, Infallible>> for ImapError {
//     fn from(err: FlagError<'_, Infallible>) -> Self {
//         ImapError::Operation(format!("Flag error: {}", err))
//     }
// }


