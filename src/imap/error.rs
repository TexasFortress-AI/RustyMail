use thiserror::Error;

/// Error type for IMAP operations
#[derive(Error, Debug, Clone)]
pub enum ImapError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Mailbox operation error: {0}")]
    Mailbox(String),
    
    #[error("Fetch error: {0}")]
    Fetch(String),
    
    #[error("Search error: {0}")]
    Search(String),
    
    #[error("Append error: {0}")]
    Append(String),
    
    #[error("UID operation error: {0}")]
    Uid(String),
    
    #[error("Invalid command or sequence: {0}")]
    Command(String),
    
    #[error("Logout error: {0}")]
    Logout(String),
    
    #[error("Error from async-imap: {0}")]
    AsyncImap(#[from] async_imap::error::Error),
    
    #[error("Error converting string to flag: {0}")]
    InvalidFlag(String),
    
    // Add other potential error sources as needed
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    
    // Add missing variants
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Invalid UID specified: {0}")]
    InvalidUid(u32),
    
    #[error("Parse error: {0}")]
    Parse(String),
}

// No longer need From<Infallible> or specific imap-next command errors
