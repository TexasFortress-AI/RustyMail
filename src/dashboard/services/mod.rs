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
use log::{info, error, warn};
use actix_web::{web::Data};
use tokio::sync::Mutex as TokioMutex;
// Import CloneableImapSessionFactory from prelude
use crate::prelude::CloneableImapSessionFactory;
use crate::connection_pool::ConnectionPool;

pub mod ai;
pub mod clients;
pub mod config;
pub mod events;
pub mod event_integration;
pub mod health;
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
pub use events::{EventBus, DashboardEvent};
pub use health::{HealthService, HealthReport, HealthStatus};

// Import the types that were causing privacy issues directly from their source
// Removed unresolved ImapConfiguration import
// Removed unused imports: ClientInfo, PaginatedClients, ChatbotQuery, ChatbotResponse, DashboardStats
// Removed unused import: ClientQueryParams
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
    pub event_bus: Arc<EventBus>,
    pub health_service: Option<Arc<HealthService>>,
    pub config: web::Data<Settings>,
    pub imap_session_factory: CloneableImapSessionFactory,
    pub connection_pool: Arc<ConnectionPool>,
}

// Initialize the services
pub async fn init(
    config: Data<crate::config::Settings>,
    imap_session_factory: CloneableImapSessionFactory,
    connection_pool: Arc<ConnectionPool>,
) -> Data<DashboardState> {
    info!("Initializing dashboard services...");

    let metrics_interval_duration = Duration::from_secs(5); // Default to 5 seconds interval
    info!("Dashboard metrics interval: {} seconds", metrics_interval_duration.as_secs());

    let _http_client = Client::new(); // Unused for now
    let client_manager = Arc::new(ClientManager::new(metrics_interval_duration));
    let metrics_service = Arc::new(MetricsService::new(metrics_interval_duration)); // Pass interval duration, not client manager
    let config_service = Arc::new(ConfigService::new());

    // Initialize AI Service with environment variables
    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();
    let morpheus_api_key = std::env::var("MORPHEUS_API_KEY").ok();
    let ollama_base_url = std::env::var("OLLAMA_API_BASE").ok();

    let ai_service = match AiService::new(openai_api_key, openrouter_api_key, morpheus_api_key, ollama_base_url).await {
        Ok(service) => Arc::new(service),
        Err(e) => {
            warn!("Failed to initialize AI service with API keys: {}. Using mock service.", e);
            Arc::new(AiService::new_mock())
        }
    };

    // Create event bus
    let event_bus = Arc::new(EventBus::new());

    // Create SSE manager and configure it with event bus
    let mut sse_manager = SseManager::new(
        metrics_service.clone(),
        client_manager.clone(),
    );
    sse_manager.set_event_bus(Arc::clone(&event_bus));
    let sse_manager = Arc::new(sse_manager);

    // Create health service
    let health_service = Arc::new(
        HealthService::new()
            .with_event_bus(Arc::clone(&event_bus))
            .with_connection_pool(Arc::clone(&connection_pool))
    );

    info!("Dashboard services initialized.");

    Data::new(DashboardState {
        client_manager,
        metrics_service,
        config_service,
        ai_service,
        sse_manager,
        event_bus,
        health_service: Some(health_service),
        config, // Pass the web::Data<Settings>
        imap_session_factory,
        connection_pool,
    })
}
