use thiserror::Error;

/// Error type for IMAP operations
#[derive(Error, Debug)]
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
}

impl From<std::io::Error> for ImapError {
    fn from(err: std::io::Error) -> Self {
        // Distinguish between general I/O and connection-specific I/O
        ImapError::Connection(format!("Underlying I/O error: {}", err))
    }
}

impl From<rustls::Error> for ImapError {
    fn from(err: rustls::Error) -> Self {
        ImapError::Connection(format!("Underlying TLS error: {}", err))
    }
}

impl From<std::string::FromUtf8Error> for ImapError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        // Correct conversion
        ImapError::Utf8(err)
    }
}

// No longer need From<Infallible> or specific imap-next command errors
