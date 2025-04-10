use std::fmt;
use std::error::Error;
use async_imap::error::Error as AsyncImapError;

#[derive(Debug)]
pub enum ImapError {
    ConnectionError(String),
    AuthenticationError(String),
    OperationError(String),
    ParseError(String),
    Encoding(String),
    Internal(String),
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
            AsyncImapError::No(msg) => ImapError::OperationError(msg),
            AsyncImapError::Bad(msg) => ImapError::OperationError(msg),
            AsyncImapError::Bye(msg) => ImapError::ConnectionError(msg),
            AsyncImapError::ConnectionError(e) => ImapError::ConnectionError(e.to_string()),
            AsyncImapError::TlsError(e) => ImapError::ConnectionError(e.to_string()),
            AsyncImapError::IoError(e) => ImapError::ConnectionError(e.to_string()),
            _ => ImapError::Internal("Unknown error".to_string()),
        }
    }
}


