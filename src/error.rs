// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Unified error handling module for RustyMail
//!
//! This module provides comprehensive error handling with JSON-RPC 2.0 compliance,
//! structured error details, and proper error mapping from async-imap and other sources.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use crate::imap::error::ImapError;
use crate::mcp::error_codes::ErrorCode;

/// Structured error details that provide context about the failed operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// The operation that failed (e.g., "list_folders", "fetch_email")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,

    /// The parameters that were provided to the operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,

    /// Additional context about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// The source error message if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Stack trace or error chain if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<String>>,
}

impl ErrorDetails {
    /// Creates a new ErrorDetails with just an operation name
    pub fn new(operation: impl Into<String>) -> Self {
        ErrorDetails {
            operation: Some(operation.into()),
            params: None,
            context: None,
            source: None,
            trace: None,
        }
    }

    /// Adds parameters to the error details
    pub fn with_params(mut self, params: Value) -> Self {
        self.params = Some(params);
        self
    }

    /// Adds context to the error details
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Adds source error message
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Adds error trace
    pub fn with_trace(mut self, trace: Vec<String>) -> Self {
        self.trace = Some(trace);
        self
    }
}

/// Maps an ImapError to an ErrorCode with structured details
pub struct ErrorMapper;

impl ErrorMapper {
    /// Maps an ImapError to the appropriate ErrorCode
    pub fn imap_to_error_code(err: &ImapError) -> ErrorCode {
        match err {
            ImapError::Connection(_) => ErrorCode::ImapConnectionError,
            ImapError::Tls(_) => ErrorCode::ImapConnectionError,
            ImapError::Auth(_) => ErrorCode::ImapAuthError,
            ImapError::InvalidMailbox(_) => ErrorCode::ImapInvalidMailbox,
            ImapError::FolderNotFound(_) => ErrorCode::ImapFolderNotFound,
            ImapError::FolderExists(_) => ErrorCode::ImapFolderExists,
            ImapError::EmailNotFound(_) => ErrorCode::ImapEmailNotFound,
            ImapError::EnvelopeNotFound | ImapError::NoEnvelope => ErrorCode::ImapEnvelopeNotFound,
            ImapError::FolderNotSelected | ImapError::RequiresFolderSelection(_) => ErrorCode::ImapFolderNotSelected,
            ImapError::Fetch(_) => ErrorCode::ImapOperationError,
            ImapError::Operation(_) => ErrorCode::ImapOperationError,
            ImapError::Command(_) => ErrorCode::ImapCommandError,
            ImapError::Flag(_) => ErrorCode::ImapInvalidFlag,
            ImapError::InvalidCriteria(_) => ErrorCode::ImapInvalidSearchCriteria,
            ImapError::Parse(_) => ErrorCode::ParseError,
            ImapError::BadResponse(_) => ErrorCode::ImapBadResponse,
            ImapError::MissingData(_) | ImapError::NoBodies => ErrorCode::ImapMessageError,
            ImapError::OperationFailed(_) => ErrorCode::ImapOperationFailed,
            ImapError::Internal(_) => ErrorCode::InternalError,
            ImapError::Timeout(_) => ErrorCode::ImapTimeoutError,
            ImapError::Io(_) => ErrorCode::ImapConnectionError,
            ImapError::Encoding(_) => ErrorCode::ParseError,
            ImapError::Validation(_) => ErrorCode::InvalidParams,
            ImapError::Other(_) | ImapError::Unknown(_) => ErrorCode::UnknownError,
        }
    }

    /// Creates structured error details from an ImapError
    pub fn imap_to_details(err: &ImapError, operation: Option<String>) -> ErrorDetails {
        let mut details = ErrorDetails {
            operation,
            params: None,
            context: None,
            source: Some(err.to_string()),
            trace: None,
        };

        // Add specific context based on error type
        match err {
            ImapError::FolderNotFound(folder) => {
                details.context = Some(serde_json::json!({
                    "folder": folder
                }));
            },
            ImapError::FolderExists(folder) => {
                details.context = Some(serde_json::json!({
                    "folder": folder
                }));
            },
            ImapError::EmailNotFound(ids) => {
                details.context = Some(serde_json::json!({
                    "message_ids": ids
                }));
            },
            ImapError::RequiresFolderSelection(op) => {
                details.context = Some(serde_json::json!({
                    "required_for_operation": op
                }));
            },
            ImapError::InvalidCriteria(criteria) => {
                details.context = Some(serde_json::json!({
                    "criteria": criteria
                }));
            },
            _ => {}
        }

        details
    }

    /// Creates a JSON-RPC error response with structured details
    pub fn to_jsonrpc_error(
        err: &ImapError,
        operation: Option<String>
    ) -> crate::mcp::types::JsonRpcError {
        let code = Self::imap_to_error_code(err);
        let details = Self::imap_to_details(err, operation);

        crate::mcp::types::JsonRpcError {
            code: code as i64,
            message: code.message().to_string(),
            data: Some(serde_json::to_value(details).unwrap_or(Value::Null)),
        }
    }
}

/// Extension trait for adding context to errors
pub trait ErrorContext {
    /// Adds operation context to the error
    fn with_operation(self, operation: impl Into<String>) -> Self;

    /// Adds parameter context to the error
    fn with_params(self, params: Value) -> Self;
}

/// Result type with our error handling
pub type RustyMailResult<T> = Result<T, RustyMailError>;

/// Main error type for RustyMail
#[derive(Debug)]
pub enum RustyMailError {
    /// IMAP-related errors
    Imap(ImapError, Option<ErrorDetails>),

    /// JSON-RPC errors
    JsonRpc(crate::mcp::types::JsonRpcError),

    /// Configuration errors
    Config(String),

    /// Session management errors
    Session(String),

    /// Other errors
    Other(String),
}

impl fmt::Display for RustyMailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustyMailError::Imap(err, _) => write!(f, "IMAP error: {}", err),
            RustyMailError::JsonRpc(err) => write!(f, "JSON-RPC error: {} ({})", err.message, err.code),
            RustyMailError::Config(msg) => write!(f, "Configuration error: {}", msg),
            RustyMailError::Session(msg) => write!(f, "Session error: {}", msg),
            RustyMailError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for RustyMailError {}

impl From<ImapError> for RustyMailError {
    fn from(err: ImapError) -> Self {
        RustyMailError::Imap(err, None)
    }
}

impl RustyMailError {
    /// Converts to a JSON-RPC error
    pub fn to_jsonrpc_error(&self, operation: Option<String>) -> crate::mcp::types::JsonRpcError {
        match self {
            RustyMailError::Imap(err, details) => {
                let code = ErrorMapper::imap_to_error_code(err);
                let error_details = details.clone()
                    .or_else(|| Some(ErrorMapper::imap_to_details(err, operation)));

                crate::mcp::types::JsonRpcError {
                    code: code as i64,
                    message: code.message().to_string(),
                    data: error_details.and_then(|d| serde_json::to_value(d).ok()),
                }
            },
            RustyMailError::JsonRpc(err) => err.clone(),
            RustyMailError::Config(msg) => crate::mcp::types::JsonRpcError {
                code: ErrorCode::InvalidParams as i64,
                message: format!("Configuration error: {}", msg),
                data: None,
            },
            RustyMailError::Session(msg) => crate::mcp::types::JsonRpcError {
                code: ErrorCode::SessionNotFound as i64,
                message: format!("Session error: {}", msg),
                data: None,
            },
            RustyMailError::Other(msg) => crate::mcp::types::JsonRpcError {
                code: ErrorCode::InternalError as i64,
                message: msg.clone(),
                data: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_error_code_mapping() {
        let err = ImapError::Auth("Invalid credentials".to_string());
        let code = ErrorMapper::imap_to_error_code(&err);
        assert_eq!(code, ErrorCode::ImapAuthError);
    }

    #[test]
    fn test_error_details_creation() {
        let details = ErrorDetails::new("list_folders")
            .with_params(json!({"session_id": "123"}))
            .with_context(json!({"server": "imap.example.com"}));

        assert_eq!(details.operation, Some("list_folders".to_string()));
        assert!(details.params.is_some());
        assert!(details.context.is_some());
    }

    #[test]
    fn test_jsonrpc_error_with_details() {
        let imap_err = ImapError::FolderNotFound("INBOX/Archive".to_string());
        let jsonrpc_err = ErrorMapper::to_jsonrpc_error(&imap_err, Some("list_folders".to_string()));

        assert_eq!(jsonrpc_err.code, ErrorCode::ImapFolderNotFound as i64);
        assert!(jsonrpc_err.data.is_some());
    }
}