// src/mcp/types.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;
// Use the ErrorCode enum from the dedicated module for consistency
use crate::mcp::error_codes::ErrorCode; 
use crate::imap::error::ImapError;

/// State relevant to a specific MCP connection or communication channel (port).
///
/// This struct holds context information that might be needed across multiple
/// requests within the same session or connection, such as the currently
/// selected IMAP folder. Maintaining this state here ensures that operations
/// dependent on context (like moving an email from the currently selected folder)
/// function correctly.
#[derive(Debug, Clone, Default)]
pub struct McpPortState {
    /// The name of the currently selected IMAP folder, if any.
    /// This is used as the default source folder for operations like `moveEmail`.
    pub selected_folder: Option<String>,
}

/// Represents a JSON-RPC 2.0 request according to the specification.
///
/// See: [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification#request_object)
#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcRequest {
    /// Must be exactly "2.0".
    pub jsonrpc: String,
    /// An identifier established by the Client that MUST contain a String, Number,
    /// or NULL value if included. If it is not included it is assumed to be a notification.
    pub id: Option<Value>,
    /// A String containing the name of the method to be invoked.
    pub method: String,
    /// A Structured value that holds the parameter values to be used
    /// during the invocation of the method. This member MAY be omitted.
    pub params: Option<Value>,
}

/// Represents a JSON-RPC 2.0 response according to the specification.
///
/// A Response is expressed as a single JSON Object, with the following members:
/// - `jsonrpc`: Must be "2.0".
/// - `result`: Required on success. MUST NOT exist if there was an error.
/// - `error`: Required on error. MUST NOT exist if there was no error.
/// - `id`: Must be the same as the value of the id member in the Request Object.
///         If there was an error in detecting the id in the Request object (e.g. Parse error/Invalid Request), it MUST be Null.
///
/// See: [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification#response_object)
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    /// Must be exactly "2.0".
    pub jsonrpc: String,
    /// Must match the `id` of the request it is responding to. `None` if the request `id` could not be determined (e.g., parse error).
    pub id: Option<Value>,
    /// The result of the method invocation. `None` if an error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// An error object if an error occurred. `None` if the request was successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Creates a JSON-RPC success response.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID from the original request.
    /// * `result` - The successful result value.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates a JSON-RPC error response.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID from the original request. `None` if the ID could not be determined.
    /// * `error` - The `JsonRpcError` object describing the error.
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Creates a response for a Parse Error (-32700).
    /// The ID is `None` as per the spec for parse errors.
    pub fn parse_error() -> Self {
        JsonRpcResponse::error(None, JsonRpcError::parse_error())
    }

    /// Creates a response for an Invalid Request Error (-32600).
    /// The ID is `None` as per the spec for invalid requests where the ID might be invalid.
    pub fn invalid_request() -> Self {
         JsonRpcResponse::error(None, JsonRpcError::invalid_request())
    }

    // Note: Method Not Found, Invalid Params, and Internal Error responses
    // usually need the request ID, so they are typically constructed directly
    // using `JsonRpcResponse::error(id, JsonRpcError::method_not_found())` etc.
}

/// Represents a JSON-RPC 2.0 error object according to the specification.
///
/// See: [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification#error_object)
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    /// A Number that indicates the error type that occurred.
    /// Must be an integer. Standard codes are defined, and -32000 to -32099 are reserved for implementation-defined server-errors.
    pub code: i32,
    /// A String providing a short description of the error.
    pub message: String,
    /// A Primitive or Structured value that contains additional information about the error.
    /// May be omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC 2.0 Error Codes and MCP/IMAP specific codes
// Use the ErrorCode enum defined in src/mcp/error_codes.rs for consistency.
// These constants can be deprecated or removed if ErrorCode is used everywhere.
/*
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // -32000 to -32099: Server error codes reserved for implementation-defined errors.
    pub const IMAP_CONNECTION_ERROR: i32 = -32000;
    pub const IMAP_AUTH_ERROR: i32 = -32001;
    // NOTE: These specific codes seem better represented by ErrorCode::ImapOperationError
    // Consider simplifying the mapping logic.
    pub const FOLDER_NOT_FOUND: i32 = -32002;
    pub const FOLDER_ALREADY_EXISTS: i32 = -32003;
    pub const EMAIL_NOT_FOUND: i32 = -32004;
    pub const IMAP_OPERATION_FAILED: i32 = -32010; // Generic IMAP failure
}
*/

impl JsonRpcError {
    /// Creates a Parse Error (-32700).
    pub fn parse_error() -> Self {
        JsonRpcError {
            code: ErrorCode::ParseError as i32,
            message: ErrorCode::ParseError.message().to_string(),
            data: None,
        }
    }

    /// Creates an Invalid Request Error (-32600).
    pub fn invalid_request() -> Self {
         JsonRpcError {
            code: ErrorCode::InvalidRequest as i32,
            message: ErrorCode::InvalidRequest.message().to_string(),
            data: None,
        }
    }

    /// Creates a Method Not Found Error (-32601).
    pub fn method_not_found() -> Self {
        JsonRpcError {
            code: ErrorCode::MethodNotFound as i32,
            message: ErrorCode::MethodNotFound.message().to_string(),
            data: None,
        }
    }

    /// Creates an Invalid Params Error (-32602).
    /// Uses a generic message by default, but can be customized.
    pub fn invalid_params<S: Into<String>>(details: S) -> Self {
        let details_str = details.into();
        let message = if details_str.is_empty() {
            ErrorCode::InvalidParams.message().to_string()
        } else {
            format!("{}: {}", ErrorCode::InvalidParams.message(), details_str)
        };
        JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message,
            data: None, // Optionally include details in data: Some(Value::String(details_str)),
        }
    }

     /// Creates an Internal Error (-32603).
     /// Uses a generic message by default, but can be customized.
    pub fn internal_error<S: Into<String>>(details: S) -> Self {
        let details_str = details.into();
        let message = if details_str.is_empty() {
            ErrorCode::InternalError.message().to_string()
        } else {
             format!("{}: {}", ErrorCode::InternalError.message(), details_str)
        };
        JsonRpcError {
            code: ErrorCode::InternalError as i32,
            message,
            // Include the detailed internal error message in the 'data' field for debugging.
            data: Some(Value::String(details_str)),
        }
    }

    /// Creates a `JsonRpcError` from an `ImapError`.
    /// This function maps specific IMAP domain errors to appropriate
    /// JSON-RPC error codes within the implementation-defined server error range (-32000 to -32099).
    pub fn from_imap_error(err: &ImapError) -> Self {
         let (error_code, detailed_message) = map_imap_err_to_mcp(err);
         JsonRpcError {
             code: error_code as i32,
             // Use the standard message for the code, put details in 'data'.
             message: error_code.message().to_string(), 
             data: Some(Value::String(detailed_message)), // Include specific IMAP error details in data
         }
     }
}

/// Maps an `ImapError` to a tuple containing the most appropriate `ErrorCode`
/// and a detailed error message string.
///
/// This helps translate internal IMAP issues into standardized MCP/JSON-RPC errors.
fn map_imap_err_to_mcp(err: &ImapError) -> (ErrorCode, String) {
    match err {
        // Connection and Auth errors
        ImapError::Connection(msg) | ImapError::ConnectionError(msg) | ImapError::Tls(msg) =>
            (ErrorCode::ImapConnectionError, format!("Connection/TLS Error: {}", msg)),
        ImapError::Auth(msg) | ImapError::AuthenticationError(msg) =>
            (ErrorCode::ImapAuthenticationError, format!("Authentication Error: {}", msg)),
        
        // Specific operation errors related to missing entities
        ImapError::FolderNotFound(name) =>
            (ErrorCode::ImapOperationError, format!("Folder not found: {}", name)), // Use generic operation error
        ImapError::FolderExists(name) =>
            (ErrorCode::ImapOperationError, format!("Folder already exists: {}", name)), // Use generic operation error
        ImapError::EmailNotFound(uids) =>
             (ErrorCode::ImapOperationError, format!("Email not found for UIDs: {:?}", uids)), // Use generic operation error
        ImapError::EnvelopeNotFound =>
             (ErrorCode::ImapOperationError, "Envelope data not found in fetch response".to_string()),
        
        // Errors related to required context (like selected folder)
        ImapError::FolderNotSelected | ImapError::RequiresFolderSelection(_) =>
            (ErrorCode::ImapOperationError, "Operation requires a folder to be selected".to_string()),

        // General operation failures
        ImapError::Mailbox(msg) | ImapError::Fetch(msg) | ImapError::Append(msg) | ImapError::Operation(msg) | ImapError::OperationFailed(msg) | ImapError::Command(msg) =>
            (ErrorCode::ImapOperationError, format!("Operation Failed: {}", msg)),
        
        // Invalid input from the client side (map to InvalidParams)
        ImapError::InvalidCriteria(crit) =>
             (ErrorCode::InvalidParams, format!("Invalid search criteria provided: {:?}", crit)),

        // Lower-level or unexpected errors (map to Internal or specific IMAP internal codes)
        ImapError::Parse(msg) | ImapError::ParseError(msg) =>
             (ErrorCode::ImapParseError, format!("IMAP Protocol Parse Error: {}", msg)),
        ImapError::BadResponse(msg) =>
             (ErrorCode::ImapInternalError, format!("Bad Server Response: {}", msg)), // Server misbehaved
        ImapError::Io(msg) =>
             (ErrorCode::ImapInternalError, format!("IMAP IO Error: {}", msg)), // Treat IO as internal
        ImapError::SessionError(e) =>
            (ErrorCode::ImapInternalError, format!("Underlying IMAP Session Error: {}", e)),
        ImapError::Encoding(msg) =>
            (ErrorCode::ImapEncodingError, format!("Internal Encoding Error: {}", msg)),
        ImapError::Config(msg) =>
            (ErrorCode::InternalError, format!("Internal Configuration Error: {}", msg)), // General internal error
        ImapError::Internal(msg) =>
            (ErrorCode::InternalError, format!("Internal Server Error: {}", msg)), // General internal error
    }
}

// Implement From trait for convenience
impl From<ImapError> for JsonRpcError {
    fn from(err: ImapError) -> Self {
        JsonRpcError::from_imap_error(&err)
    }
}

// Implement std::error::Error for JsonRpcError for better integration
impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC Error (code {}): {}", self.code, self.message)?;
        if let Some(data) = &self.data {
            write!(f, " (Data: {})", data)?;
        }
        Ok(())
    }
}
impl std::error::Error for JsonRpcError {} 