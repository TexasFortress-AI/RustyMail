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
pub use routes::init_routes;
pub use errors::ApiError;
