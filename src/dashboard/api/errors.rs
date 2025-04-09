use actix_web::{
    error::ResponseError,
    http::StatusCode,
    HttpResponse,
};
use serde::Serialize;
use thiserror::Error;
use crate::imap::error::ImapError;
use log;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapError),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    status: u16,
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();
        let error_message = self.to_string();
        
        // Log internal errors with more detail
        if status_code == StatusCode::INTERNAL_SERVER_ERROR {
            log::error!("Dashboard error: {:?}", self);
        } else {
            log::warn!("Dashboard error: {:?}", self);
        }
        
        HttpResponse::build(status_code).json(ErrorResponse {
            error: error_message,
            status: status_code.as_u16(),
        })
    }
    
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::ImapError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::SerializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
