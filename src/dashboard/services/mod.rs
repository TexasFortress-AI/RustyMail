// Dashboard Services module
//
// This module contains the core services for the dashboard functionality:
// - Metrics collection
// - Client management 
// - Configuration management
// - AI assistant integration

use thiserror::Error;
use std::sync::Arc;
use actix_web::web;
use log::{info, error};
use actix_web::{web::Data};
use tokio::sync::Mutex as TokioMutex;
// Import the factory from the correct module
use crate::imap::ImapSessionFactory;

pub mod ai;
pub mod clients;
pub mod config;
pub mod metrics;

// Define or import error types if they exist
#[derive(Error, Debug)] pub enum MetricsError { #[error("Metrics collection failed: {0}")] CollectionFailed(String), #[error("Metrics storage error: {0}")] StorageError(String) }
#[derive(Error, Debug)] pub enum ClientError { #[error("Client not found: {0}")] NotFound(String), #[error("Client operation failed: {0}")] OperationFailed(String) }
#[derive(Error, Debug)] pub enum ConfigError { #[error("Configuration loading failed: {0}")] LoadError(String), #[error("Configuration update failed: {0}")] UpdateError(String) }

// Import API models that might be needed
// Removed unresolved ImapConfiguration import
// use crate::dashboard::api::models::{ImapConfiguration}; 

// Re-export main service types for convenience
pub use metrics::{MetricsService};
pub use clients::{ClientManager};
pub use config::{ConfigService};
pub use ai::{AiService};

// Import the types that were causing privacy issues directly from their source
// Removed unresolved ImapConfiguration import
use crate::dashboard::api::models::{ClientInfo, PaginatedClients, ChatbotQuery, ChatbotResponse, DashboardStats};
use crate::dashboard::api::handlers::ClientQueryParams;
// Removed unused ApiError import
// use crate::api::rest::ApiError;
use crate::config::Settings;
use crate::dashboard::api::sse::SseManager;
// Removed unused ImapClient import
// use crate::imap::client::ImapClient;
// Corrected factory path
use reqwest::Client;
// Added missing imports
use std::time::Duration;

// Import provider trait for mock
// use crate::dashboard::services::ai::provider::AiProvider;

// Shared state for dashboard services
#[derive(Clone)]
pub struct DashboardState {
    pub client_manager: Arc<ClientManager>,
    pub metrics_service: Arc<MetricsService>,
    pub config_service: Arc<ConfigService>,
    pub ai_service: Arc<AiService>,
    pub sse_manager: Arc<SseManager>,
    pub config: web::Data<Settings>,
    pub imap_session_factory: ImapSessionFactory,
}

// Initialize the services
pub fn init(
    config: Data<crate::config::Settings>,
    imap_session_factory: ImapSessionFactory,
) -> Data<DashboardState> {
    info!("Initializing dashboard services...");

    let metrics_interval_duration = Duration::from_secs(config.dashboard.as_ref().map_or(5, |d| d.metrics_interval));
    info!("Dashboard metrics interval: {} seconds", metrics_interval_duration.as_secs());

    let http_client = Client::new();
    let client_manager = Arc::new(ClientManager::new(metrics_interval_duration));
    let metrics_service = Arc::new(MetricsService::new(client_manager.clone())); // Pass client_manager to metrics
    let config_service = Arc::new(ConfigService::new());

    // Initialize AI Service (handle potential errors)
    let ai_service: Arc<AiService> = match AiService::new(&config.ai, http_client.clone()) {
        Ok(service) => Arc::new(service),
        Err(e) => {
            error!("Failed to initialize AiService: {}. AI features will be disabled or use mock.", e);
            // Fallback to a mock or disabled service if needed
            // For now, creating a default/potentially non-functional one
            // This might need a specific MockAiService or similar depending on requirements
            Arc::new(AiService::default()) // Assuming AiService implements Default
        }
    };

    let sse_manager = Arc::new(SseManager::new(
        metrics_service.clone(),
        client_manager.clone(),
    ));

    info!("Dashboard services initialized.");

    Data::new(DashboardState {
        client_manager,
        metrics_service,
        config_service,
        ai_service, 
        sse_manager,
        config, // Pass the web::Data<Settings>
        imap_session_factory,
    })
}
