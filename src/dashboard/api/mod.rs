// This file is no longer needed as its logic was moved to main.rs
// and dashboard/api/routes.rs

// Dashboard API module

pub mod accounts;
pub mod routes;
pub mod sse;
pub mod models;
pub mod handlers;
pub mod errors;
pub mod middleware;
pub mod config;
pub mod health;

// Re-export main types needed elsewhere
pub use routes::configure as init_routes;
pub use sse::SseManager;

// The init function previously here is no longer needed, 
// its logic moved to main.rs
