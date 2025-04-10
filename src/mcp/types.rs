// src/mcp/types.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::imap::error::ImapError;

/// State relevant to an MCP connection/port.
#[derive(Debug, Clone, Default)] 
pub struct McpPortState {
    /// The currently selected IMAP folder for context-sensitive operations.
    pub selected_folder: Option<String>,
}

/// Represents a JSON-RPC 2.0 request.
#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// Represents a JSON-RPC 2.0 response.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Creates a success response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error response.
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Creates a response for a Parse Error (-32700).
    pub fn parse_error() -> Self {
        JsonRpcResponse::error(None, JsonRpcError::parse_error())
    }

    /// Creates a response for an Invalid Request Error (-32600).
    pub fn invalid_request() -> Self {
         JsonRpcResponse::error(None, JsonRpcError::invalid_request())
    }

    // Note: Method Not Found, Invalid Params, and Internal Error responses
    // usually need the request ID, so they are typically constructed directly
    // using JsonRpcResponse::error(id, JsonRpcError::method_not_found()) etc.
}

/// Represents a JSON-RPC 2.0 error object.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC 2.0 Error Codes
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // -32000 to -32099: Server error codes reserved for implementation-defined errors.
    pub const IMAP_CONNECTION_ERROR: i32 = -32000;
    pub const IMAP_AUTH_ERROR: i32 = -32001;
    pub const FOLDER_NOT_FOUND: i32 = -32002;
    pub const FOLDER_ALREADY_EXISTS: i32 = -32003;
    pub const EMAIL_NOT_FOUND: i32 = -32004;
    pub const IMAP_OPERATION_FAILED: i32 = -32010; // Generic IMAP failure
}

impl JsonRpcError {
    /// Creates a Parse Error (-32700).
    pub fn parse_error() -> Self {
        JsonRpcError {
            code: codes::PARSE_ERROR,
            message: "Parse error".to_string(),
            data: None,
        }
    }

    /// Creates an Invalid Request Error (-32600).
    pub fn invalid_request() -> Self {
         JsonRpcError {
            code: codes::INVALID_REQUEST,
            message: "Invalid Request".to_string(),
            data: None,
        }
    }

    /// Creates a Method Not Found Error (-32601).
    pub fn method_not_found() -> Self {
        JsonRpcError {
            code: codes::METHOD_NOT_FOUND,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    /// Creates an Invalid Params Error (-32602).
    pub fn invalid_params<S: Into<String>>(message: S) -> Self {
        JsonRpcError {
            code: codes::INVALID_PARAMS,
            message: message.into(), // Use provided message
            data: None,
        }
    }

     /// Creates an Internal Error (-32603).
    pub fn internal_error<S: Into<String>>(message: S) -> Self {
        JsonRpcError {
            code: codes::INTERNAL_ERROR,
            message: message.into(), // Use provided message
            data: None,
        }
    }

    // Implementation-defined server errors
    pub fn from_imap_error(err: ImapError) -> Self {
         let (code, message) = map_imap_err_to_mcp(&err);
         JsonRpcError {
             code,
             message,
             data: Some(Value::String(format!("{:?}", err))), // Include original error details in data
         }
     }

}

// Helper function to map ImapError variants to JSON-RPC error codes and messages
// Updated based on the actual ImapError definition
fn map_imap_err_to_mcp(err: &ImapError) -> (i32, String) {
    match err {
        // Map specific, well-defined IMAP errors to specific MCP codes
        ImapError::Connection(msg) | ImapError::ConnectionError(msg) | ImapError::Tls(msg) =>
            (codes::IMAP_CONNECTION_ERROR, format!("IMAP Connection/TLS Error: {}", msg)),
        ImapError::Auth(msg) | ImapError::AuthenticationError(msg) =>
            (codes::IMAP_AUTH_ERROR, format!("IMAP Authentication Error: {}", msg)),
        ImapError::FolderNotFound(name) =>
            (codes::FOLDER_NOT_FOUND, format!("Folder not found: {}", name)),
        ImapError::FolderExists(name) =>
            (codes::FOLDER_ALREADY_EXISTS, format!("Folder already exists: {}", name)),
        ImapError::EmailNotFound(uids) =>
             (codes::EMAIL_NOT_FOUND, format!("Email not found for UIDs: {:?}", uids)),
        ImapError::FolderNotSelected | ImapError::RequiresFolderSelection(_) =>
            (codes::IMAP_OPERATION_FAILED, "Operation requires a folder to be selected".to_string()), // Or maybe a specific code?

        // Map broader/generic IMAP errors
        ImapError::Mailbox(msg) | ImapError::Fetch(msg) | ImapError::Append(msg) | ImapError::Operation(msg) | ImapError::OperationFailed(msg) | ImapError::Command(msg) =>
            (codes::IMAP_OPERATION_FAILED, format!("IMAP Operation Failed: {}", msg)),
        ImapError::InvalidCriteria(crit) =>
             (codes::INVALID_PARAMS, format!("Invalid search criteria: {:?}", crit)), // Map to Invalid Params

        // Map lower-level errors
        ImapError::Parse(msg) | ImapError::ParseError(msg) =>
             (codes::INTERNAL_ERROR, format!("IMAP Parse Error: {}", msg)),
        ImapError::BadResponse(msg) =>
             (codes::INTERNAL_ERROR, format!("IMAP Bad Server Response: {}", msg)),
        ImapError::Io(msg) =>
             (codes::INTERNAL_ERROR, format!("IMAP IO Error: {}", msg)),
        ImapError::SessionError(e) =>
            (codes::INTERNAL_ERROR, format!("IMAP Session Error: {}", e)), // Keep internal for underlying library errors
        ImapError::Encoding(msg) =>
            (codes::INTERNAL_ERROR, format!("Internal Encoding Error: {}", msg)),
        ImapError::Config(msg) =>
            (codes::INTERNAL_ERROR, format!("Internal Configuration Error: {}", msg)),
        ImapError::Internal(msg) =>
            (codes::INTERNAL_ERROR, format!("Internal Server Error: {}", msg)),
        ImapError::EnvelopeNotFound => // Should this map to EMAIL_NOT_FOUND?
             (codes::IMAP_OPERATION_FAILED, "Envelope not found".to_string()),
    }
}

// Implement From trait for convenience
impl From<ImapError> for JsonRpcError {
    fn from(err: ImapError) -> Self {
        JsonRpcError::from_imap_error(err)
    }
}

// Implement std::error::Error for JsonRpcError for better integration
impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC Error (code {}): {}", self.code, self.message)
    }
}
impl std::error::Error for JsonRpcError {} 