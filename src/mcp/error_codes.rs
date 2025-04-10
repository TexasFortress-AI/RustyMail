use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ErrorCode {
    // Standard JSON-RPC 2.0 error codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    
    // IMAP-specific error codes
    ImapConnectionError = -32000,
    ImapAuthenticationError = -32001,
    ImapOperationError = -32002,
    ImapParseError = -32003,
    ImapEncodingError = -32004,
    ImapInternalError = -32005,
    
    // MCP-specific error codes
    McpConnectionError = -32100,
    McpAuthenticationError = -32101,
    McpOperationError = -32102,
    McpParseError = -32103,
    McpEncodingError = -32104,
    McpInternalError = -32105,
}

impl ErrorCode {
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
            
            ErrorCode::ImapConnectionError => "IMAP connection error",
            ErrorCode::ImapAuthenticationError => "IMAP authentication error",
            ErrorCode::ImapOperationError => "IMAP operation error",
            ErrorCode::ImapParseError => "IMAP parse error",
            ErrorCode::ImapEncodingError => "IMAP encoding error",
            ErrorCode::ImapInternalError => "IMAP internal error",
            
            ErrorCode::McpConnectionError => "MCP connection error",
            ErrorCode::McpAuthenticationError => "MCP authentication error",
            ErrorCode::McpOperationError => "MCP operation error",
            ErrorCode::McpParseError => "MCP parse error",
            ErrorCode::McpEncodingError => "MCP encoding error",
            ErrorCode::McpInternalError => "MCP internal error",
        }
    }
}

// Constants for common error codes
pub const PARSE_ERROR: i32 = ErrorCode::ParseError as i32;
pub const INVALID_REQUEST: i32 = ErrorCode::InvalidRequest as i32;
pub const METHOD_NOT_FOUND: i32 = ErrorCode::MethodNotFound as i32;
pub const INVALID_PARAMS: i32 = ErrorCode::InvalidParams as i32;
pub const INTERNAL_ERROR: i32 = ErrorCode::InternalError as i32; 