//! Defines the standard JSON-RPC 2.0 error codes and custom
//! MCP/IMAP specific codes used within RustyMail.

use serde::{Deserialize, Serialize};

/// Defines the standard and implementation-specific error codes used in the MCP protocol.
///
/// The error codes follow the JSON-RPC 2.0 specification:
/// - Standard JSON-RPC codes: -32700 to -32600
/// - Implementation-defined server errors: -32000 to -32099
///   - These are further subdivided into IMAP-specific and MCP-specific ranges.
///
/// See: [JSON-RPC 2.0 Specification - Error Object](https://www.jsonrpc.org/specification#error_object)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // Standard JSON-RPC 2.0 error codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    
    // IMAP-specific error codes (implementation-defined range)
    ImapConnectionError = -32000,
    ImapAuthError = -32001, 
    ImapFolderNotFound = -32002,
    ImapFolderExists = -32003,
    ImapEmailNotFound = -32004,
    ImapEnvelopeNotFound = -32005,
    ImapFolderNotSelected = -32006,
    ImapOperationError = -32007,
    ImapInvalidFlag = -32008,
    ImapInvalidSearchCriteria = -32009,
    ImapBadResponse = -32010,
    ImapTimeoutError = -32011,
    ImapCommandError = -32012,
    ImapInvalidMailbox = -32013,
    ImapOperationFailed = -32014,
    ImapMessageError = -32015,

    // MCP-specific error codes
    McpInvalidRequest = -32050,
    McpInvalidParams = -32051,
    McpMethodNotFound = -32052,
    McpInternalError = -32053,
    McpParseError = -32054,
    
    // Session errors
    SessionNotFound = -32080,
    SessionCreationFailed = -32081,
    SessionAccessDenied = -32082,
    
    // General errors
    UnknownError = -32099
}

impl ErrorCode {
    /// Returns the standard descriptive message for the error code.
    pub fn message(&self) -> &'static str {
        match self {
            // Standard JSON-RPC 2.0 error messages
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
            
            // IMAP-specific error messages
            ErrorCode::ImapConnectionError => "IMAP: Connection error",
            ErrorCode::ImapAuthError => "IMAP: Authentication error",
            ErrorCode::ImapFolderNotFound => "IMAP: Folder not found",
            ErrorCode::ImapFolderExists => "IMAP: Folder already exists",
            ErrorCode::ImapEmailNotFound => "IMAP: Email not found",
            ErrorCode::ImapEnvelopeNotFound => "IMAP: Envelope not found",
            ErrorCode::ImapFolderNotSelected => "IMAP: No folder selected",
            ErrorCode::ImapOperationError => "IMAP: Operation error",
            ErrorCode::ImapInvalidFlag => "IMAP: Invalid flag",
            ErrorCode::ImapInvalidSearchCriteria => "IMAP: Invalid search criteria",
            ErrorCode::ImapBadResponse => "IMAP: Bad response",
            ErrorCode::ImapTimeoutError => "IMAP: Operation timed out",
            ErrorCode::ImapCommandError => "IMAP: Command error",
            ErrorCode::ImapInvalidMailbox => "IMAP: Invalid mailbox",
            ErrorCode::ImapOperationFailed => "IMAP: Operation failed",
            ErrorCode::ImapMessageError => "IMAP: Message error",

            // MCP-specific error messages
            ErrorCode::McpInvalidRequest => "MCP: Invalid request",
            ErrorCode::McpInvalidParams => "MCP: Invalid parameters",
            ErrorCode::McpMethodNotFound => "MCP: Method not found",
            ErrorCode::McpInternalError => "MCP: Internal error",
            ErrorCode::McpParseError => "MCP: Parse error",
            
            // Session error messages
            ErrorCode::SessionNotFound => "Session not found",
            ErrorCode::SessionCreationFailed => "Failed to create session",
            ErrorCode::SessionAccessDenied => "Session access denied",
            
            // General error messages
            ErrorCode::UnknownError => "Unknown error",
        }
    }
}

// Constants for common error codes (for backward compatibility)
pub const PARSE_ERROR: i32 = ErrorCode::ParseError as i32;
pub const INVALID_REQUEST: i32 = ErrorCode::InvalidRequest as i32;
pub const METHOD_NOT_FOUND: i32 = ErrorCode::MethodNotFound as i32;
pub const INVALID_PARAMS: i32 = ErrorCode::InvalidParams as i32;
pub const INTERNAL_ERROR: i32 = ErrorCode::InternalError as i32; 