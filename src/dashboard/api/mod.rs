// Dashboard API module
//
// This module contains the REST API endpoints and SSE implementation
// for the RustyMail dashboard

pub mod routes;
pub mod handlers;
pub mod models;
pub mod errors;
pub mod sse;

// Re-export main types
pub use routes::configure as init_routes;
pub use errors::ApiError;
pub use sse::SseManager;

use actix_web::web;
use std::sync::Arc;

// Initialize the API module
pub fn init(cfg: &mut actix_web::web::ServiceConfig) {
    // Configure routes
    routes::configure(cfg);
    
    // Initialize SSE manager 
    let sse_manager = Arc::new(sse::SseManager::new());
    let sse_manager_data = web::Data::new(sse_manager);
    
    // Register SSE manager with the app
    cfg.app_data(sse_manager_data);
}
