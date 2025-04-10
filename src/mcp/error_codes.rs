use serde::{Deserialize, Serialize};

/// Defines the standard and implementation-specific error codes used in the MCP protocol.
///
/// The error codes follow the JSON-RPC 2.0 specification:
/// - Standard JSON-RPC codes: -32700 to -32600
/// - Implementation-defined server errors: -32000 to -32099
///   - These are further subdivided into IMAP-specific and MCP-specific ranges.
///
/// See: [JSON-RPC 2.0 Specification - Error Object](https://www.jsonrpc.org/specification#error_object)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Standard JSON-RPC 2.0 error codes
    /// Invalid JSON was received by the server.
    /// An error occurred on the server while parsing the JSON text.
    ParseError = -32700,
    /// The JSON sent is not a valid Request object.
    InvalidRequest = -32600,
    /// The method does not exist / is not available.
    MethodNotFound = -32601,
    /// Invalid method parameter(s).
    InvalidParams = -32602,
    /// Internal JSON-RPC error.
    InternalError = -32603,
    
    // IMAP-specific error codes
    /// Error related to establishing or maintaining the connection to the IMAP server.
    ImapConnectionError = -32000,
    /// Authentication with the IMAP server failed (e.g., wrong username/password).
    ImapAuthenticationError = -32001,
    /// A general error occurred during an IMAP operation (e.g., folder not found, email exists).
    /// Specific details should be in the error message or data.
    ImapOperationError = -32002,
    /// An error occurred while parsing the IMAP server's response.
    ImapParseError = -32003,
    /// An error related to character encoding during IMAP communication.
    ImapEncodingError = -32004,
    /// An unexpected or internal error originating from the IMAP client library or interaction.
    ImapInternalError = -32005,
    
    // MCP-specific error codes
    /// Error related to the MCP transport connection (e.g., Stdio pipe broken, SSE connection lost).
    McpConnectionError = -32050,
    /// Authentication failure specific to the MCP layer (if applicable).
    McpAuthenticationError = -32051,
    /// A general error occurred during an MCP operation itself, not mapped from IMAP.
    McpOperationError = -32052,
    /// Error parsing the MCP message structure (distinct from JSON ParseError).
    McpParseError = -32053,
    /// Error related to character encoding within the MCP layer.
    McpEncodingError = -32054,
    /// An internal error within the MCP framework logic.
    McpInternalError = -32055,
}

impl ErrorCode {
    /// Returns the standard descriptive message for the error code.
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
            
            ErrorCode::ImapConnectionError => "IMAP: Connection error",
            ErrorCode::ImapAuthenticationError => "IMAP: Authentication error",
            ErrorCode::ImapOperationError => "IMAP: Operation error",
            ErrorCode::ImapParseError => "IMAP: Protocol parse error",
            ErrorCode::ImapEncodingError => "IMAP: Encoding error",
            ErrorCode::ImapInternalError => "IMAP: Internal error",
            
            ErrorCode::McpConnectionError => "MCP: Connection error",
            ErrorCode::McpAuthenticationError => "MCP: Authentication error",
            ErrorCode::McpOperationError => "MCP: Operation error",
            ErrorCode::McpParseError => "MCP: Parse error",
            ErrorCode::McpEncodingError => "MCP: Encoding error",
            ErrorCode::McpInternalError => "MCP: Internal error",
        }
    }
}

// Constants for common error codes
pub const PARSE_ERROR: i32 = ErrorCode::ParseError as i32;
pub const INVALID_REQUEST: i32 = ErrorCode::InvalidRequest as i32;
pub const METHOD_NOT_FOUND: i32 = ErrorCode::MethodNotFound as i32;
pub const INVALID_PARAMS: i32 = ErrorCode::InvalidParams as i32;
pub const INTERNAL_ERROR: i32 = ErrorCode::InternalError as i32; 