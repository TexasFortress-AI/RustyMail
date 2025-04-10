use std::error::Error as StdError;
use thiserror::Error;
use async_imap::error::Error as AsyncImapError;
use imap_types::error::ValidationError as FlagValidationError;
use imap_types::flag;
use std::fmt;
use async_imap;
use tokio_native_tls;

#[derive(Debug, Error)]
pub enum ImapError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("TLS error: {0}")]
    Tls(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Invalid mailbox: {0}")]
    InvalidMailbox(String),
    
    #[error("Folder not found: {0}")]
    FolderNotFound(String),
    
    #[error("Folder already exists: {0}")]
    FolderExists(String),
    
    #[error("Email not found: {0:?}")]
    EmailNotFound(Vec<u32>),
    
    #[error("Envelope not found")]
    EnvelopeNotFound,
    
    #[error("Folder not selected")]
    FolderNotSelected,
    
    #[error("Operation requires folder selection: {0}")]
    RequiresFolderSelection(String),
    
    #[error("Fetch error: {0}")]
    Fetch(String),
    
    #[error("Operation error: {0}")]
    Operation(String),
    
    #[error("Command error: {0}")]
    Command(String),
    
    #[error("Flag error: {0}")]
    Flag(String),
    
    #[error("Invalid search criteria: {0}")]
    InvalidCriteria(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Bad response: {0}")]
    BadResponse(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Operation timed out")]
    Timeout,
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<async_imap::error::Error> for ImapError {
    fn from(err: async_imap::error::Error) -> Self {
        match err {
            async_imap::error::Error::Parse(e) => ImapError::Parse(e.to_string()),
            async_imap::error::Error::No(msg) => ImapError::Operation(msg),
            async_imap::error::Error::Bad(msg) => ImapError::BadResponse(msg),
            async_imap::error::Error::Io(e) => ImapError::Connection(e.to_string()),
            async_imap::error::Error::Validate(e) => ImapError::Command(e.to_string()),
            _ => ImapError::Unknown(err.to_string()),
        }
    }
}

impl From<tokio_native_tls::native_tls::Error> for ImapError {
    fn from(err: tokio_native_tls::native_tls::Error) -> Self {
        ImapError::Tls(err.to_string())
    }
}

impl From<std::io::Error> for ImapError {
    fn from(err: std::io::Error) -> Self {
        ImapError::Connection(err.to_string())
    }
}

impl From<flag::ValidationError> for ImapError {
    fn from(err: flag::ValidationError) -> Self {
        ImapError::Flag(err.to_string())
    }
}



