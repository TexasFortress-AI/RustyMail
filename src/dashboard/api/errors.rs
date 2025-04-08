use actix_web::{HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use serde_json::json;
use thiserror::Error;
use std::fmt;
use crate::imap::error::ImapError;

#[derive(Error, Debug)]
pub enum DashboardError {
    #[error("Bad Request: {0}")]
    BadRequest(String),
    
    #[error("Not Found: {0}")]
    NotFound(String),
    
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    
    #[error("Service Unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("IMAP Error: {0}")]
    ImapError(#[from] ImapError),
    
    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl ResponseError for DashboardError {
    fn status_code(&self) -> StatusCode {
        match self {
            DashboardError::BadRequest(_) => StatusCode::BAD_REQUEST,
            DashboardError::NotFound(_) => StatusCode::NOT_FOUND,
            DashboardError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DashboardError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            DashboardError::ImapError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DashboardError::SerializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        
        // Log internal errors with more detail
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            log::error!("Dashboard error: {:?}", self);
        } else {
            log::warn!("Dashboard error: {:?}", self);
        }
        
        HttpResponse::build(status)
            .json(json!({
                "error": self.to_string(),
                "code": status.as_u16()
            }))
    }
}
