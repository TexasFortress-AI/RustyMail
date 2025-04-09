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
use crate::dashboard::api::SseManager;
use crate::imap::ImapClient;

// Shared state for dashboard services
#[derive(Clone)]
pub struct DashboardState {
    pub metrics_service: Arc<MetricsService>,
    pub client_manager: Arc<ClientManager>,
    pub config_service: Arc<ConfigService>,
    pub ai_service: Arc<AiService>,
    pub sse_manager: Arc<SseManager>,
    pub imap_client: Arc<ImapClient>,
}

// Initialize the services
pub fn init(
    _config: web::Data<Settings>,
    imap_client: Arc<ImapClient>
) -> web::Data<DashboardState> {
    info!("Initializing dashboard services");
    
    // Create services (Metrics, Client, Config, AI)
    let metrics_service = Arc::new(MetricsService::new(Duration::from_secs(5)));
    let client_manager = Arc::new(ClientManager::new(Duration::from_secs(600)));
    let config_service = Arc::new(ConfigService::new());
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    let ai_service = Arc::new(AiService::new(api_key));

    // Create SseManager (requires metrics and client manager)
    let sse_manager = Arc::new(SseManager::new(
        metrics_service.clone(),
        client_manager.clone(),
    ));
    
    // Create dashboard state including SseManager and ImapClient
    let state = DashboardState {
        metrics_service,
        client_manager,
        config_service,
        ai_service,
        sse_manager,
        imap_client,
    };
    
    web::Data::new(state)
}
