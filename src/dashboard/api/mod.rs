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
use crate::dashboard::services::DashboardState;

// Initialize the API module
pub fn init(cfg: &mut actix_web::web::ServiceConfig, dashboard_state: web::Data<DashboardState>) {
    // Configure routes
    routes::configure(cfg);
    
    // Initialize SSE manager with services from dashboard state
    let sse_manager = Arc::new(sse::SseManager::new(
        Arc::clone(&dashboard_state.metrics_service),
        Arc::clone(&dashboard_state.client_manager)
    ));
    let sse_manager_data = web::Data::new(Arc::clone(&sse_manager));
    
    // Register SSE manager with the app
    cfg.app_data(sse_manager_data);
    
    // Start stats broadcast
    let sse_manager_clone = Arc::clone(&sse_manager);
    actix_web::rt::spawn(async move {
        sse_manager_clone.start_stats_broadcast(dashboard_state).await;
    });
}
