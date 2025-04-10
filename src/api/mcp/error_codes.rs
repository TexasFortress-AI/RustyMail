#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // General errors
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ParseError = -32700,

    // IMAP-specific errors
    ImapConnectionError = -32000,
    ImapAuthenticationError = -32001,
    ImapOperationError = -32002,
    ImapTimeoutError = -32003,
    ImapProtocolError = -32004,
}

impl ErrorCode {
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid parameters",
            ErrorCode::InternalError => "Internal error",
            ErrorCode::ParseError => "Parse error",
            ErrorCode::ImapConnectionError => "IMAP connection error",
            ErrorCode::ImapAuthenticationError => "IMAP authentication error",
            ErrorCode::ImapOperationError => "IMAP operation error",
            ErrorCode::ImapTimeoutError => "IMAP timeout error",
            ErrorCode::ImapProtocolError => "IMAP protocol error",
        }
    }
} 