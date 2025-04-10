// This file is no longer needed as its logic was moved to main.rs
// and dashboard/api/routes.rs

// Dashboard API module

pub mod routes;
pub mod sse;
pub mod models;
pub mod handlers;
pub mod errors;
pub mod middleware;

// Re-export main types needed elsewhere
// pub use routes::configure as init_routes; // Unused
// pub use sse::SseManager; // Unused

// The init function previously here is no longer needed, 
// its logic moved to main.rs
