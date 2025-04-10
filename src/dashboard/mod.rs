// Dashboard module for RustyMail SSE Dashboard
//
// This module contains all dashboard-related functionality including:
// - API endpoints for the frontend
// - Services for metrics, client management, and AI
// - SSE implementation for real-time updates

//! Dashboard feature module.

// Re-export key dashboard components
pub mod api;
pub mod services;
pub mod testing; // Integration tests module

// pub use api::models::{DashboardStats, ClientInfo, PaginatedClients}; // Unused
// pub use api::routes::configure as init_api_routes; // Unused
// pub use services::metrics::MetricsService; // Unused
// pub use services::clients::ClientManager; // Unused
pub use services::DashboardState;

// Comment out the duplicate testing module
// pub mod testing;

// The init function previously here is no longer needed,
// its logic moved to main.rs
