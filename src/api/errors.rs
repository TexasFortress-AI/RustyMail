//! Comprehensive error handling module for the REST API
//!
//! This module provides a unified error handling system with:
//! - Standardized error response format
//! - Consistent HTTP status code mapping
//! - Detailed error context and tracing
//! - Client-friendly error messages

use actix_web::{
    error::ResponseError,
    http::StatusCode,
    HttpResponse,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dashboard::api::errors::ApiError as DashboardApiError,
    imap::error::ImapError,
};

/// Standardized error response format
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code for programmatic handling
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Additional error details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
    /// Request ID for tracing (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Timestamp of the error
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Additional error details
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Field-specific validation errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_errors: Option<Vec<ValidationError>>,
    /// Suggested actions to resolve the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
    /// Related documentation links
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_links: Option<Vec<String>>,
}

/// Field-specific validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Field name that failed validation
    pub field: String,
    /// Validation error message
    pub message: String,
    /// Validation constraint that failed (e.g., "required", "min_length")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
}

/// Main API error enum with comprehensive error types
#[derive(Debug, Error)]
pub enum ApiError {
    // === Authentication & Authorization Errors (401, 403) ===
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid API key: {reason}")]
    InvalidApiKey { reason: String },

    #[error("Insufficient permissions: {required_scope}")]
    Forbidden { required_scope: String },

    #[error("API key expired")]
    ApiKeyExpired,

    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },

    // === Validation Errors (400) ===
    #[error("Validation failed: {message}")]
    ValidationFailed {
        message: String,
        errors: Vec<ValidationError>,
    },

    #[error("Invalid request: {message}")]
    BadRequest { message: String },

    #[error("Invalid query parameter: {param}")]
    InvalidQueryParam { param: String, reason: String },

    #[error("Invalid path parameter: {param}")]
    InvalidPathParam { param: String, reason: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid field value: {field}")]
    InvalidFieldValue { field: String, reason: String },

    // === Resource Errors (404, 409, 410) ===
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Folder not found: {folder}")]
    FolderNotFound { folder: String },

    #[error("Email not found: UID {uid}")]
    EmailNotFound { uid: u32 },

    #[error("Resource already exists: {resource}")]
    Conflict { resource: String },

    #[error("Resource no longer available: {resource}")]
    Gone { resource: String },

    // === Server & External Service Errors (500, 502, 503) ===
    #[error("Internal server error: {message}")]
    InternalError { message: String },

    #[error("IMAP connection error: {message}")]
    ImapConnection { message: String },

    #[error("IMAP operation failed: {operation}")]
    ImapOperation { operation: String, details: String },

    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("External service unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Gateway timeout: {service}")]
    GatewayTimeout { service: String },

    // === Content Errors (413, 415, 422) ===
    #[error("Payload too large: max size is {max_size} bytes")]
    PayloadTooLarge { max_size: usize },

    #[error("Unsupported media type: {media_type}")]
    UnsupportedMediaType { media_type: String },

    #[error("Unprocessable entity: {message}")]
    UnprocessableEntity { message: String },

    // === Method & Operation Errors (405, 501) ===
    #[error("Method not allowed: {method}")]
    MethodNotAllowed { method: String },

    #[error("Operation not implemented: {operation}")]
    NotImplemented { operation: String },
}

impl ApiError {
    /// Get the error code for programmatic handling
    pub fn code(&self) -> String {
        match self {
            // Authentication
            ApiError::Unauthorized => "AUTH_REQUIRED".to_string(),
            ApiError::InvalidApiKey { .. } => "INVALID_API_KEY".to_string(),
            ApiError::Forbidden { .. } => "FORBIDDEN".to_string(),
            ApiError::ApiKeyExpired => "API_KEY_EXPIRED".to_string(),
            ApiError::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED".to_string(),

            // Validation
            ApiError::ValidationFailed { .. } => "VALIDATION_FAILED".to_string(),
            ApiError::BadRequest { .. } => "BAD_REQUEST".to_string(),
            ApiError::InvalidQueryParam { .. } => "INVALID_QUERY_PARAM".to_string(),
            ApiError::InvalidPathParam { .. } => "INVALID_PATH_PARAM".to_string(),
            ApiError::MissingField { .. } => "MISSING_FIELD".to_string(),
            ApiError::InvalidFieldValue { .. } => "INVALID_FIELD_VALUE".to_string(),

            // Resources
            ApiError::NotFound { .. } => "NOT_FOUND".to_string(),
            ApiError::FolderNotFound { .. } => "FOLDER_NOT_FOUND".to_string(),
            ApiError::EmailNotFound { .. } => "EMAIL_NOT_FOUND".to_string(),
            ApiError::Conflict { .. } => "CONFLICT".to_string(),
            ApiError::Gone { .. } => "GONE".to_string(),

            // Server
            ApiError::InternalError { .. } => "INTERNAL_ERROR".to_string(),
            ApiError::ImapConnection { .. } => "IMAP_CONNECTION_ERROR".to_string(),
            ApiError::ImapOperation { .. } => "IMAP_OPERATION_ERROR".to_string(),
            ApiError::DatabaseError { .. } => "DATABASE_ERROR".to_string(),
            ApiError::ServiceUnavailable { .. } => "SERVICE_UNAVAILABLE".to_string(),
            ApiError::GatewayTimeout { .. } => "GATEWAY_TIMEOUT".to_string(),

            // Content
            ApiError::PayloadTooLarge { .. } => "PAYLOAD_TOO_LARGE".to_string(),
            ApiError::UnsupportedMediaType { .. } => "UNSUPPORTED_MEDIA_TYPE".to_string(),
            ApiError::UnprocessableEntity { .. } => "UNPROCESSABLE_ENTITY".to_string(),

            // Methods
            ApiError::MethodNotAllowed { .. } => "METHOD_NOT_ALLOWED".to_string(),
            ApiError::NotImplemented { .. } => "NOT_IMPLEMENTED".to_string(),
        }
    }

    /// Get suggested actions for the error
    pub fn suggestions(&self) -> Option<Vec<String>> {
        match self {
            ApiError::Unauthorized => Some(vec![
                "Include a valid API key in the X-API-Key header".to_string(),
            ]),
            ApiError::InvalidApiKey { .. } => Some(vec![
                "Check your API key is correct".to_string(),
                "Generate a new API key if needed".to_string(),
            ]),
            ApiError::RateLimitExceeded { .. } => Some(vec![
                "Wait before making more requests".to_string(),
                "Consider implementing request batching".to_string(),
            ]),
            ApiError::ValidationFailed { .. } => Some(vec![
                "Review the validation errors for each field".to_string(),
                "Ensure all required fields are provided".to_string(),
            ]),
            ApiError::FolderNotFound { folder } => Some(vec![
                format!("Check if folder '{}' exists", folder),
                "List available folders with GET /api/v1/folders".to_string(),
            ]),
            ApiError::PayloadTooLarge { max_size } => Some(vec![
                format!("Reduce payload size to under {} bytes", max_size),
                "Consider chunking large requests".to_string(),
            ]),
            _ => None,
        }
    }

    /// Get help links for the error
    pub fn help_links(&self) -> Option<Vec<String>> {
        match self {
            ApiError::Unauthorized | ApiError::InvalidApiKey { .. } => Some(vec![
                "/docs/authentication".to_string(),
            ]),
            ApiError::ValidationFailed { .. } | ApiError::BadRequest { .. } => Some(vec![
                "/docs/api-reference".to_string(),
            ]),
            ApiError::RateLimitExceeded { .. } => Some(vec![
                "/docs/rate-limits".to_string(),
            ]),
            _ => None,
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            // 400 Bad Request
            ApiError::BadRequest { .. } |
            ApiError::ValidationFailed { .. } |
            ApiError::InvalidQueryParam { .. } |
            ApiError::InvalidPathParam { .. } |
            ApiError::MissingField { .. } |
            ApiError::InvalidFieldValue { .. } => StatusCode::BAD_REQUEST,

            // 401 Unauthorized
            ApiError::Unauthorized |
            ApiError::InvalidApiKey { .. } |
            ApiError::ApiKeyExpired => StatusCode::UNAUTHORIZED,

            // 403 Forbidden
            ApiError::Forbidden { .. } => StatusCode::FORBIDDEN,

            // 404 Not Found
            ApiError::NotFound { .. } |
            ApiError::FolderNotFound { .. } |
            ApiError::EmailNotFound { .. } => StatusCode::NOT_FOUND,

            // 405 Method Not Allowed
            ApiError::MethodNotAllowed { .. } => StatusCode::METHOD_NOT_ALLOWED,

            // 409 Conflict
            ApiError::Conflict { .. } => StatusCode::CONFLICT,

            // 410 Gone
            ApiError::Gone { .. } => StatusCode::GONE,

            // 413 Payload Too Large
            ApiError::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,

            // 415 Unsupported Media Type
            ApiError::UnsupportedMediaType { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,

            // 422 Unprocessable Entity
            ApiError::UnprocessableEntity { .. } => StatusCode::UNPROCESSABLE_ENTITY,

            // 429 Too Many Requests
            ApiError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,

            // 500 Internal Server Error
            ApiError::InternalError { .. } |
            ApiError::ImapConnection { .. } |
            ApiError::ImapOperation { .. } |
            ApiError::DatabaseError { .. } => StatusCode::INTERNAL_SERVER_ERROR,

            // 501 Not Implemented
            ApiError::NotImplemented { .. } => StatusCode::NOT_IMPLEMENTED,

            // 502 Bad Gateway
            ApiError::GatewayTimeout { .. } => StatusCode::BAD_GATEWAY,

            // 503 Service Unavailable
            ApiError::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        // Log errors appropriately
        match status.as_u16() {
            400..=499 => log::warn!("Client error: {} ({})", self, status),
            500..=599 => log::error!("Server error: {} ({})", self, status),
            _ => log::info!("API response: {} ({})", self, status),
        }

        // Build error details
        let mut details = ErrorDetails {
            validation_errors: None,
            suggestions: self.suggestions(),
            help_links: self.help_links(),
        };

        // Add validation errors if present
        if let ApiError::ValidationFailed { errors, .. } = self {
            details.validation_errors = Some(errors.to_vec());
        }

        let error_response = ErrorResponse {
            code: self.code(),
            message: self.to_string(),
            details: if details.validation_errors.is_some()
                || details.suggestions.is_some()
                || details.help_links.is_some() {
                Some(details)
            } else {
                None
            },
            request_id: None, // TODO: Add request ID from middleware
            timestamp: chrono::Utc::now(),
        };

        HttpResponse::build(status).json(error_response)
    }
}

// === Type Conversions ===

impl From<ImapError> for ApiError {
    fn from(err: ImapError) -> Self {
        match err {
            ImapError::Connection(msg) => ApiError::ImapConnection { message: msg },
            ImapError::Auth(msg) => ApiError::InvalidApiKey { reason: msg },
            ImapError::FolderNotFound(folder) => ApiError::FolderNotFound { folder },
            ImapError::EmailNotFound(uids) => ApiError::EmailNotFound {
                uid: uids.first().copied().unwrap_or(0)
            },
            ImapError::FolderExists(folder) => ApiError::Conflict {
                resource: format!("Folder '{}'", folder)
            },
            ImapError::Tls(msg) | ImapError::InvalidMailbox(msg) => ApiError::ImapConnection {
                message: msg
            },
            _ => ApiError::InternalError { message: err.to_string() },
        }
    }
}

impl From<DashboardApiError> for ApiError {
    fn from(err: DashboardApiError) -> Self {
        ApiError::InternalError { message: format!("Dashboard error: {}", err) }
    }
}

impl From<validator::ValidationErrors> for ApiError {
    fn from(errors: validator::ValidationErrors) -> Self {
        let validation_errors: Vec<ValidationError> = errors
            .field_errors()
            .iter()
            .flat_map(|(field, field_errors)| {
                field_errors.iter().map(|e| ValidationError {
                    field: field.to_string(),
                    message: e.message.as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| e.code.to_string()),
                    constraint: Some(e.code.to_string()),
                })
            })
            .collect();

        ApiError::ValidationFailed {
            message: "Request validation failed".to_string(),
            errors: validation_errors,
        }
    }
}

// === Helper Functions ===

/// Create a standardized success response
pub fn success_response<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": data,
        "timestamp": chrono::Utc::now(),
    }))
}

/// Create a standardized paginated response
pub fn paginated_response<T: Serialize>(
    data: Vec<T>,
    total: usize,
    limit: usize,
    offset: usize,
) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": data,
        "pagination": {
            "total": total,
            "limit": limit,
            "offset": offset,
            "has_more": offset + data.len() < total,
        },
        "timestamp": chrono::Utc::now(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(ApiError::Unauthorized.code(), "AUTH_REQUIRED");
        assert_eq!(
            ApiError::ValidationFailed {
                message: "test".to_string(),
                errors: vec![]
            }.code(),
            "VALIDATION_FAILED"
        );
        assert_eq!(
            ApiError::FolderNotFound {
                folder: "test".to_string()
            }.code(),
            "FOLDER_NOT_FOUND"
        );
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(ApiError::Unauthorized.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            ApiError::BadRequest { message: "test".to_string() }.status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::NotFound { resource: "test".to_string() }.status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::InternalError { message: "test".to_string() }.status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ApiError::RateLimitExceeded { message: "test".to_string() }.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_suggestions() {
        let auth_error = ApiError::Unauthorized;
        assert!(auth_error.suggestions().is_some());
        assert!(auth_error.suggestions().unwrap().len() > 0);

        let rate_error = ApiError::RateLimitExceeded { message: "test".to_string() };
        assert!(rate_error.suggestions().is_some());

        let internal_error = ApiError::InternalError { message: "test".to_string() };
        assert!(internal_error.suggestions().is_none());
    }

    #[test]
    fn test_validation_error_conversion() {
        use validator::Validate;

        #[derive(Validate)]
        struct TestStruct {
            #[validate(length(min = 1))]
            field: String,
        }

        let test = TestStruct { field: "".to_string() };
        let validation_result = test.validate();
        assert!(validation_result.is_err());

        let api_error: ApiError = validation_result.unwrap_err().into();
        if let ApiError::ValidationFailed { errors, .. } = api_error {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].field, "field");
        } else {
            panic!("Expected ValidationFailed error");
        }
    }
}