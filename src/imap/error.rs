use thiserror::Error;
// Remove FlagError import as it doesn't seem to exist
// use imap_types::flag::FlagError;
// Remove unused Infallible
// use std::convert::Infallible;
use crate::imap::types::SearchCriteria;

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
    Io(String),
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error("Envelope Not Found")]
    EnvelopeNotFound,

    // --- Add missing specific variants ---
    #[error("Folder Not Found: {0}")]
    FolderNotFound(String),
    #[error("Folder Already Exists: {0}")]
    FolderExists(String),
    #[error("Requires Folder Selection: {0}")]
    RequiresFolderSelection(String),

    // Add other specific errors as needed from async-imap or imap-types
    #[error("IMAP connection failed: {0}")]
    ConnectionError(String),
    #[error("IMAP authentication failed: {0}")]
    AuthenticationError(String),
    #[error("IMAP email not found for UIDs: {0:?}")]
    EmailNotFound(Vec<u32>),
    #[error("IMAP operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid search criteria provided: {0:?}")]
    InvalidCriteria(SearchCriteria),
    #[error("Operation requires a folder to be selected.")]
    FolderNotSelected,
    #[error("Failed to parse IMAP response: {0}")]
    ParseError(String),
    #[error("IMAP session error: {0}")]
    SessionError(#[from] async_imap::error::Error),
}

impl From<std::io::Error> for ImapError {
    fn from(err: std::io::Error) -> Self {
        ImapError::ConnectionError(format!("IO Error: {}", err))
    }
}

// Remove the manual From<std::io::Error> as #[from] handles it.
// impl From<std::io::Error> for ImapError { ... }

// Remove From<rustls::Error> for now, can be added later if required.
// impl From<tokio_rustls::rustls::Error> for ImapError { ... }

// Remove From implementation for FlagError
// impl From<FlagError<'_, Infallible>> for ImapError {
//     fn from(err: FlagError<'_, Infallible>) -> Self {
//         ImapError::Operation(format!("Flag error: {}", err))
//     }
// }


