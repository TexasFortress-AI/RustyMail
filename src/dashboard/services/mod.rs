// Dashboard Services module
//
// This module contains the core services for the dashboard functionality:
// - Metrics collection
// - Client management 
// - Configuration management
// - AI assistant integration

pub mod metrics;
pub mod clients;
pub mod config;
pub mod ai;

// Re-export main service types for convenience
pub use metrics::MetricsService;
pub use clients::ClientManager;
pub use config::ConfigService;
pub use ai::AiService;
