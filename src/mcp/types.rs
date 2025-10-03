// src/mcp/types.rs

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
// Use the ErrorCode enum from the dedicated module for consistency
use crate::mcp::error_codes::ErrorCode; 
use crate::imap::error::ImapError;
use std::fmt;
use tokio::sync::Mutex;
use log;
use crate::session_manager::SessionManager;
use crate::dashboard::services::cache::CacheService;
use std::sync::Arc;

// Error code constants for IMAP errors - these match the enum values in ErrorCode
pub const CODE_IMAP_CONNECTION_ERROR: i64 = ErrorCode::ImapConnectionError as i64;
pub const CODE_IMAP_AUTH_ERROR: i64 = ErrorCode::ImapAuthError as i64;
pub const CODE_IMAP_FOLDER_NOT_FOUND: i64 = ErrorCode::ImapFolderNotFound as i64;
pub const CODE_IMAP_FOLDER_EXISTS: i64 = ErrorCode::ImapFolderExists as i64;
pub const CODE_IMAP_EMAIL_NOT_FOUND: i64 = ErrorCode::ImapEmailNotFound as i64;
pub const CODE_IMAP_ENVELOPE_NOT_FOUND: i64 = ErrorCode::ImapEnvelopeNotFound as i64;
pub const CODE_IMAP_FOLDER_NOT_SELECTED: i64 = ErrorCode::ImapFolderNotSelected as i64;
pub const CODE_IMAP_OPERATION_ERROR: i64 = ErrorCode::ImapOperationError as i64;
pub const CODE_IMAP_INVALID_FLAG: i64 = ErrorCode::ImapInvalidFlag as i64;
pub const CODE_IMAP_INVALID_SEARCH_CRITERIA: i64 = ErrorCode::ImapInvalidSearchCriteria as i64;
pub const CODE_IMAP_BAD_RESPONSE: i64 = ErrorCode::ImapBadResponse as i64;
pub const CODE_IMAP_TIMEOUT_ERROR: i64 = ErrorCode::ImapTimeoutError as i64;
pub const CODE_IMAP_COMMAND_ERROR: i64 = ErrorCode::ImapCommandError as i64;
pub const CODE_IMAP_INVALID_MAILBOX: i64 = ErrorCode::ImapInvalidMailbox as i64;
pub const CODE_IMAP_PARSE_ERROR: i64 = -32020; // Custom code for IMAP parse errors
pub const CODE_IMAP_IO_ERROR: i64 = -32021; // Custom code for IMAP IO errors
pub const CODE_IMAP_TLS_ERROR: i64 = -32022; // Custom code for IMAP TLS errors
pub const CODE_IMAP_UNKNOWN_ERROR: i64 = -32023; // Custom code for unknown IMAP errors
pub const CODE_IMAP_INTERNAL_ERROR: i64 = -32024; // Custom code for internal IMAP errors
pub const CODE_IMAP_MISSING_DATA: i64 = -32025; // Custom code for missing data errors
pub const CODE_IMAP_FETCH_ERROR: i64 = -32026; // Custom code for fetch errors
pub const CODE_IMAP_ENCODING_ERROR: i64 = -32027; // Custom code for encoding errors
pub const CODE_IMAP_VALIDATION_ERROR: i64 = -32028; // Custom code for validation errors
pub const CODE_IMAP_OPERATION_FAILED: i64 = -32029; // Custom code for operation failed errors
pub const CODE_IMAP_REQUIRES_FOLDER_SELECTION: i64 = -32030; // Custom code for operations requiring folder selection

// JSON-RPC standard error codes
pub const ERROR_PARSE: i64 = ErrorCode::ParseError as i64;
pub const ERROR_INVALID_REQUEST: i64 = ErrorCode::InvalidRequest as i64;
pub const ERROR_METHOD_NOT_FOUND: i64 = ErrorCode::MethodNotFound as i64;
pub const ERROR_INVALID_PARAMS: i64 = ErrorCode::InvalidParams as i64;
pub const ERROR_INTERNAL: i64 = ErrorCode::InternalError as i64;

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
    /// The ID of the currently selected account for MCP operations.
    /// This mirrors the account context pattern used in the web UI.
    pub current_account_id: Option<String>,
    session_id: Option<String>,
    session_manager: Arc<SessionManager>,
    /// Cache service for database operations
    pub cache_service: Option<Arc<CacheService>>,
}

impl McpPortState {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            selected_folder: None,
            current_account_id: None,
            session_id: None,
            session_manager,
            cache_service: None,
        }
    }

    /// Create a new state with cache service
    pub fn with_cache_service(session_manager: Arc<SessionManager>, cache_service: Arc<CacheService>) -> Self {
        Self {
            selected_folder: None,
            current_account_id: None,
            session_id: None,
            session_manager,
            cache_service: Some(cache_service),
        }
    }
    
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
    }
    
    pub fn get_session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }
    
    pub fn get_session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    // Disabled - MockSessionManager not compatible with public API
    // #[cfg(test)]
    // pub fn new_mock() -> Self {
    //     use crate::session_manager::MockSessionManager;
    //     Self {
    //         selected_folder: None,
    //         session_id: Some("mock_session".to_string()),
    //         session_manager: Arc::new(MockSessionManager::new()),
    //     }
    // }
}

/// Represents a JSON-RPC 2.0 request according to the specification.
///
/// See: [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification#request_object)
#[derive(Deserialize, Serialize, Debug, Clone)]
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
#[derive(Serialize, Deserialize, Debug, Clone)]
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcError {
    /// A Number that indicates the error type that occurred.
    /// Must be an integer. Standard codes are defined, and -32000 to -32099 are reserved for implementation-defined server-errors.
    pub code: i64,
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
            code: ErrorCode::ParseError as i64,
            message: ErrorCode::ParseError.message().to_string(),
            data: None,
        }
    }

    /// Creates an Invalid Request Error (-32600).
    pub fn invalid_request() -> Self {
         JsonRpcError {
            code: ErrorCode::InvalidRequest as i64,
            message: ErrorCode::InvalidRequest.message().to_string(),
            data: None,
        }
    }

    /// Creates a Method Not Found Error (-32601).
    pub fn method_not_found() -> Self {
        JsonRpcError {
            code: ErrorCode::MethodNotFound as i64,
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
            code: ErrorCode::InvalidParams as i64,
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
            code: ErrorCode::InternalError as i64,
            message,
            // Include the detailed internal error message in the 'data' field for debugging.
            data: Some(Value::String(details_str)),
        }
    }

    /// Creates a server error response with the specified code and message.
    pub fn server_error(code: i64, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}

/// Maps an `ImapError` to a tuple containing the most appropriate `ErrorCode`
/// and a detailed error message string.
///
/// This helps translate internal IMAP issues into standardized MCP/JSON-RPC errors.
fn map_imap_err_to_mcp(err: &ImapError) -> (i64, String) {
    match err {
        ImapError::Connection(msg) => 
            (ErrorCode::ImapConnectionError as i64, format!("Connection error: {}", msg)),
        ImapError::Auth(msg) => 
            (ErrorCode::ImapAuthError as i64, format!("Authentication error: {}", msg)),
        ImapError::Parse(msg) =>
            (ErrorCode::ParseError as i64, format!("Parse error: {}", msg)),
        ImapError::Validation(msg) =>
            (ErrorCode::InvalidParams as i64, format!("Validation error: {}", msg)),
        ImapError::Command(msg) => 
            (ErrorCode::ImapCommandError as i64, format!("Command error: {}", msg)),
        ImapError::InvalidCriteria(crit) => 
            (ErrorCode::ImapInvalidSearchCriteria as i64, format!("Invalid search criteria: {}", crit)),
        ImapError::Timeout(msg) =>
            (ErrorCode::InternalError as i64, format!("Timeout: {}", msg)),
        ImapError::NoBodies => 
            (ErrorCode::ImapMessageError as i64, "No message bodies found".to_string()),
        ImapError::NoEnvelope => 
            (ErrorCode::ImapMessageError as i64, "No envelope found".to_string()),
        ImapError::Operation(msg) => 
            (ErrorCode::ImapOperationError as i64, format!("Operation error: {}", msg)),
        ImapError::OperationFailed(msg) => 
            (ErrorCode::ImapOperationFailed as i64, format!("Operation failed: {}", msg)),
        ImapError::FolderNotFound(folder) => 
            (ErrorCode::ImapFolderNotFound as i64, format!("Folder not found: {}", folder)),
        ImapError::InvalidMailbox(msg) => 
            (ErrorCode::ImapFolderNotFound as i64, format!("Invalid mailbox: {}", msg)),
        ImapError::Other(msg) =>
            (ErrorCode::UnknownError as i64, format!("Unknown error: {}", msg)),
        // Catch-all for any other variants
        _ =>
            (ErrorCode::InternalError as i64, "Internal IMAP error".to_string()),
    }
}

// Implement From trait for convenience
impl From<ImapError> for JsonRpcError {
    fn from(err: ImapError) -> Self {
        // Use the unified error mapper from the error module
        crate::error::ErrorMapper::to_jsonrpc_error(&err, None)
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