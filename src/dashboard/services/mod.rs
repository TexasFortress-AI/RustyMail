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

use actix_web::web;
use std::sync::Arc;
use std::time::Duration;
use crate::config::Settings;
use log::info;

// Shared state for dashboard services
pub struct DashboardState {
    pub metrics_service: Arc<MetricsService>,
    pub client_manager: Arc<ClientManager>,
    pub config_service: Arc<ConfigService>,
    pub ai_service: Arc<AiService>,
}

// Initialize the services
pub fn init(config: web::Data<Settings>) -> web::Data<DashboardState> {
    info!("Initializing dashboard services");
    
    // Create metrics service with 5-second update interval
    let metrics_service = Arc::new(MetricsService::new(Duration::from_secs(5)));
    
    // Create client manager with 10-minute cleanup interval
    let client_manager = Arc::new(ClientManager::new(Duration::from_secs(600)));
    
    // Create config service
    let config_service = Arc::new(ConfigService::new());
    
    // Create AI service
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    let ai_service = Arc::new(AiService::new(api_key));
    
    // Create dashboard state
    let state = DashboardState {
        metrics_service,
        client_manager,
        config_service,
        ai_service,
    };
    
    web::Data::new(state)
}
