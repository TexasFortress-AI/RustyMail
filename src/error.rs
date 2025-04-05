use thiserror::Error;
use actix_web::{ResponseError, HttpResponse, http::StatusCode, body::BoxBody};
use serde::Serialize;

#[derive(Debug, Error)]
pub enum ImapApiError {
    #[error("IMAP Connection Error: {0}")]
    ConnectionError(String),
    #[error("IMAP Authentication Error: {0}")]
    AuthError(String),
    #[error("IMAP Operation Error: {0}")]
    ImapError(String),
    #[error("Invalid Request: {0}")]
    InvalidRequest(String),
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Folder operation error: {0}")]
    FolderError(String),
    #[error("Folder not found: {0}")]
    FolderNotFound(String),
    #[error("Folder not empty: {0} messages")]
    FolderNotEmpty(u32),
    #[error("Email operation error: {0}")]
    EmailError(String),
    #[error("Email not found: {0}")]
    EmailNotFound(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("MIME error: {0}")]
    MimeError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Address parse error: {0}")]
    AddressParseError(String),
}

// Implement conversion from common error types
impl From<imap::Error> for ImapApiError {
    fn from(err: imap::Error) -> Self {
        match &err {
            imap::Error::No(msg) => ImapApiError::FolderError(msg.to_string()),
            imap::Error::Bad(msg) => ImapApiError::EmailError(msg.to_string()),
            imap::Error::Append => ImapApiError::EmailError("Failed to append message".into()),
            imap::Error::Parse(e) => ImapApiError::ParseError(e.to_string()),
            imap::Error::Validate(e) => ImapApiError::ValidationError(e.to_string()),
            imap::Error::Io(io_err) => ImapApiError::IoError(std::io::Error::new(io_err.kind(), format!("{}", io_err))),
            _ => ImapApiError::InternalError(format!("IMAP Error: {}", err)),
        }
    }
}

impl From<native_tls::Error> for ImapApiError {
    fn from(err: native_tls::Error) -> Self {
        ImapApiError::TlsError(err.to_string())
    }
}

impl From<config::ConfigError> for ImapApiError {
    fn from(err: config::ConfigError) -> Self {
        ImapApiError::ConfigError(err.to_string())
    }
}

impl From<lettre::error::Error> for ImapApiError {
    fn from(err: lettre::error::Error) -> Self {
        ImapApiError::MimeError(err.to_string())
    }
}

impl From<lettre::address::AddressError> for ImapApiError {
    fn from(err: lettre::address::AddressError) -> Self {
        ImapApiError::AddressParseError(err.to_string())
    }
}

impl From<base64::DecodeError> for ImapApiError {
    fn from(err: base64::DecodeError) -> Self {
        ImapApiError::ParseError(format!("Base64 decode error: {}", err))
    }
}

impl ResponseError for ImapApiError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            ImapApiError::ConnectionError(_) |
            ImapApiError::ImapError(_) |
            ImapApiError::TlsError(_) |
            ImapApiError::InternalError(_) |
            ImapApiError::IoError(_) |
            ImapApiError::ConfigError(_) |
            ImapApiError::MimeError(_) |
            ImapApiError::ParseError(_) => {
                HttpResponse::InternalServerError().json(ApiErrorResponse {
                    error: self.to_string(),
                })
            }
            ImapApiError::AuthError(_) => {
                HttpResponse::Unauthorized().json(ApiErrorResponse {
                    error: self.to_string(),
                })
            }
            ImapApiError::InvalidRequest(_) |
            ImapApiError::ValidationError(_) |
            ImapApiError::AddressParseError(_) => {
                HttpResponse::BadRequest().json(ApiErrorResponse {
                    error: self.to_string(),
                })
            }
            ImapApiError::NotFound(_) |
            ImapApiError::FolderNotFound(_) |
            ImapApiError::EmailNotFound(_) => {
                HttpResponse::NotFound().json(ApiErrorResponse {
                    error: self.to_string(),
                })
            }
            ImapApiError::FolderNotEmpty(count) => {
                HttpResponse::BadRequest().json(FolderNotEmptyErrorResponse {
                    error: self.to_string(),
                    message_count: *count,
                })
            }
            ImapApiError::FolderError(_) |
            ImapApiError::EmailError(_) => {
                HttpResponse::InternalServerError().json(ApiErrorResponse {
                    error: self.to_string(),
                })
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ImapApiError::ConnectionError(_) |
            ImapApiError::ImapError(_) |
            ImapApiError::TlsError(_) |
            ImapApiError::InternalError(_) |
            ImapApiError::IoError(_) |
            ImapApiError::ConfigError(_) |
            ImapApiError::MimeError(_) |
            ImapApiError::ParseError(_) |
            ImapApiError::FolderError(_) |
            ImapApiError::EmailError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ImapApiError::AuthError(_) => StatusCode::UNAUTHORIZED,
            ImapApiError::InvalidRequest(_) |
            ImapApiError::ValidationError(_) |
            ImapApiError::AddressParseError(_) |
            ImapApiError::FolderNotEmpty(_) => StatusCode::BAD_REQUEST,
            ImapApiError::NotFound(_) |
            ImapApiError::FolderNotFound(_) |
            ImapApiError::EmailNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct FolderNotEmptyErrorResponse {
    pub error: String,
    pub message_count: u32,
}
