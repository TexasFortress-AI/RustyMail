use crate::imap::client::ImapClientError;
use thiserror::Error;
use actix_web::http::StatusCode;
use actix_web::ResponseError;
use config::ConfigError;

#[derive(Debug, Error)]
pub enum ImapApiError {
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapClientError),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("MIME error: {0}")]
    MimeError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Address parse error: {0}")]
    AddressParseError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Folder not found: {0}")]
    FolderNotFound(String),
    #[error("Email not found: {0}")]
    EmailNotFound(String),
    #[error("Folder not empty: {0} emails remaining")]
    FolderNotEmpty(usize),
    #[error("Folder error: {0}")]
    FolderError(String),
    #[error("Email error: {0}")]
    EmailError(String),
}

impl ImapApiError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ImapApiError::ConnectionError(_) |
            ImapApiError::TlsError(_) |
            ImapApiError::InternalError(_) |
            ImapApiError::IoError(_) |
            ImapApiError::ConfigError(_) |
            ImapApiError::MimeError(_) |
            ImapApiError::ParseError(_) |
            ImapApiError::FolderError(_) |
            ImapApiError::EmailError(_) |
            ImapApiError::ImapError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            
            ImapApiError::AuthError(_) => StatusCode::UNAUTHORIZED,
            
            ImapApiError::InvalidRequest(_) |
            ImapApiError::AddressParseError(_) |
            ImapApiError::ValidationError(_) |
            ImapApiError::FolderNotEmpty(_) => StatusCode::BAD_REQUEST,
            
            ImapApiError::NotFound(_) |
            ImapApiError::FolderNotFound(_) |
            ImapApiError::EmailNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

impl From<ConfigError> for ImapApiError {
    fn from(err: ConfigError) -> Self {
        ImapApiError::ConfigError(err.to_string())
    }
}

impl ResponseError for ImapApiError {
    fn error_response(&self) -> actix_web::HttpResponse {
        actix_web::HttpResponse::build(self.status_code())
            .json(serde_json::json!({
                "error": self.to_string()
            }))
    }
}
