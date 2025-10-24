// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

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

    #[error("AI Service error: {0}")]
    AiServiceError(String),

    #[error("AI Service request failed: {0}")]
    AiRequestError(String),
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
        if status_code == StatusCode::INTERNAL_SERVER_ERROR || status_code == StatusCode::SERVICE_UNAVAILABLE {
            log::error!("Dashboard API error: {:?}", self);
        } else {
            log::warn!("Dashboard API error: {:?}", self);
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
            // AI Errors map to Service Unavailable or Internal Error
            ApiError::AiServiceError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::AiRequestError(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}
