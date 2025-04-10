use std::fmt;
use std::error::Error;
use async_imap::error::Error as AsyncImapError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImapError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Command error: {0}")]
    CommandError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Operation timed out")]
    Timeout,
    #[error("Resource not found")]
    NotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Quota exceeded")]
    QuotaExceeded,
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Server responded with NO: {0}")]
    No(String),
    #[error("Server responded with BAD: {0}")]
    Bad(String),
    #[error("Session error: {0}")]
    SessionError(String),
}

impl fmt::Display for ImapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImapError::ConnectionError(msg) => write!(f, "IMAP connection error: {}", msg),
            ImapError::AuthenticationError(msg) => write!(f, "IMAP authentication error: {}", msg),
            ImapError::OperationError(msg) => write!(f, "IMAP operation error: {}", msg),
            ImapError::ParseError(msg) => write!(f, "IMAP parse error: {}", msg),
            ImapError::Encoding(msg) => write!(f, "IMAP encoding error: {}", msg),
            ImapError::Internal(msg) => write!(f, "IMAP internal error: {}", msg),
        }
    }
}

impl Error for ImapError {}

impl From<AsyncImapError> for ImapError {
    fn from(error: AsyncImapError) -> Self {
        match error {
            AsyncImapError::Parse(e) => ImapError::ParseError(e.to_string()),
            AsyncImapError::Append => ImapError::OperationError("Append operation failed".to_string()),
            AsyncImapError::Create => ImapError::OperationError("Create operation failed".to_string()),
            AsyncImapError::Delete => ImapError::OperationError("Delete operation failed".to_string()),
            AsyncImapError::Examine => ImapError::OperationError("Examine operation failed".to_string()),
            AsyncImapError::Fetch => ImapError::OperationError("Fetch operation failed".to_string()),
            AsyncImapError::Login => ImapError::AuthenticationError("Login failed".to_string()),
            AsyncImapError::Logout => ImapError::OperationError("Logout failed".to_string()),
            AsyncImapError::Rename => ImapError::OperationError("Rename operation failed".to_string()),
            AsyncImapError::Search => ImapError::OperationError("Search operation failed".to_string()),
            AsyncImapError::Select => ImapError::OperationError("Select operation failed".to_string()),
            AsyncImapError::Store => ImapError::OperationError("Store operation failed".to_string()),
            AsyncImapError::Validate(e) => ImapError::ParseError(e.to_string()),
            AsyncImapError::No(msg) => ImapError::No(msg),
            AsyncImapError::Bad(msg) => ImapError::Bad(msg),
            AsyncImapError::Bye(msg) => ImapError::ConnectionError(msg),
            AsyncImapError::ConnectionError(e) => ImapError::ConnectionError(e.to_string()),
            AsyncImapError::TlsError(e) => ImapError::ConnectionError(e.to_string()),
            AsyncImapError::IoError(e) => ImapError::ConnectionError(e.to_string()),
            _ => ImapError::Internal("Unknown error".to_string()),
        }
    }
}

impl From<std::io::Error> for ImapError {
    fn from(err: std::io::Error) -> Self {
        ImapError::ConnectionError(err.to_string())
    }
}


