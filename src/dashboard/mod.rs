// Dashboard module for RustyMail SSE Dashboard
//
// This module contains all dashboard-related functionality including:
// - API endpoints for the frontend
// - Services for metrics, client management, and AI
// - SSE implementation for real-time updates

pub mod api;
pub mod services;

// Re-export commonly used types
pub use api::models::{DashboardStats, ClientInfo, PaginatedClients};
pub use api::routes::configure as init_api_routes; // Re-export route config
pub use services::metrics::MetricsService;
pub use services::clients::ClientManager;

// Only include testing in debug builds
#[cfg(test)]
pub mod testing;

// The init function previously here is no longer needed,
// its logic moved to main.rs
