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
use sqlx::SqlitePool;

pub mod account;
pub mod account_store;
pub mod ai;
pub mod cache;
pub mod clients;
pub mod config;
pub mod email;
pub mod events;
pub mod event_integration;
pub mod health;
pub mod metrics;
pub mod sync;

// Define or import error types if they exist
#[derive(Error, Debug)] pub enum MetricsError { #[error("Metrics collection failed: {0}")] CollectionFailed(String), #[error("Metrics storage error: {0}")] StorageError(String) }
#[derive(Error, Debug)] pub enum ClientError { #[error("Client not found: {0}")] NotFound(String), #[error("Client operation failed: {0}")] OperationFailed(String) }
#[derive(Error, Debug)] pub enum ConfigError { #[error("Configuration loading failed: {0}")] LoadError(String), #[error("Configuration update failed: {0}")] UpdateError(String) }

// Import API models that might be needed
// Removed unresolved ImapConfiguration import
// use crate::dashboard::api::models::{ImapConfiguration}; 

// Re-export main service types for convenience
pub use account::{AccountService, Account, ProviderTemplate, AutoConfigResult};
pub use account_store::{AccountStore, StoredAccount, ImapConfig as StoredImapConfig, SmtpConfig as StoredSmtpConfig};
pub use metrics::{MetricsService};
pub use cache::{CacheService, CacheConfig};
pub use clients::{ClientManager};
pub use config::{ConfigService};
pub use ai::{AiService};
pub use email::{EmailService};
pub use events::{EventBus, DashboardEvent};
pub use health::{HealthService, HealthReport, HealthStatus};
pub use sync::{SyncService};

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
    pub cache_service: Arc<CacheService>,
    pub config_service: Arc<ConfigService>,
    pub ai_service: Arc<AiService>,
    pub email_service: Arc<EmailService>,
    pub sync_service: Arc<SyncService>,
    pub account_service: Arc<TokioMutex<AccountService>>,
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

    // Initialize Cache Service
    let cache_config = CacheConfig {
        database_url: std::env::var("CACHE_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/email_cache.db".to_string()),
        max_memory_items: std::env::var("CACHE_MAX_MEMORY_ITEMS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
        max_cache_size_mb: std::env::var("CACHE_MAX_SIZE_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
        max_email_age_days: std::env::var("CACHE_MAX_EMAIL_AGE_DAYS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30),
        sync_interval_seconds: std::env::var("CACHE_SYNC_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300),
    };

    let mut cache_service = CacheService::new(cache_config);
    if let Err(e) = cache_service.initialize().await {
        warn!("Failed to initialize cache service: {}. Running without cache.", e);
    }
    let cache_service = Arc::new(cache_service);

    // Initialize Account Service with file-based storage
    let accounts_config_path = std::env::var("ACCOUNTS_CONFIG_PATH")
        .unwrap_or_else(|_| "config/accounts.json".to_string());

    let mut account_service_temp = AccountService::new(&accounts_config_path);

    // Get database pool for provider templates (temporary - using cache database)
    let cache_db_url = std::env::var("CACHE_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/email_cache.db".to_string());

    let account_db_pool = SqlitePool::connect(&cache_db_url)
        .await
        .expect("Failed to create database pool for account service");

    if let Err(e) = account_service_temp.initialize(account_db_pool).await {
        error!("Failed to initialize account service: {}", e);
    }

    // Auto-create account from environment variables if none exist
    if let Err(e) = account_service_temp.ensure_default_account_from_env(&config).await {
        warn!("Failed to create default account from environment: {}", e);
    }

    let account_service = Arc::new(TokioMutex::new(account_service_temp));

    // Initialize Email Service with cache and account service
    let email_service = Arc::new(
        EmailService::new(
            imap_session_factory.clone(),
            connection_pool.clone(),
        )
        .with_cache(cache_service.clone())
        .with_account_service(account_service.clone())
    );

    // Initialize Sync Service
    let sync_interval = std::env::var("SYNC_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300); // Default 5 minutes

    let sync_service = Arc::new(SyncService::new(
        imap_session_factory.clone(),
        cache_service.clone(),
        sync_interval,
    ));

    // Initialize AI Service with environment variables
    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();
    let morpheus_api_key = std::env::var("MORPHEUS_API_KEY").ok();
    let ollama_base_url = std::env::var("OLLAMA_API_BASE").ok();
    let api_key = std::env::var("RUSTYMAIL_API_KEY").ok();

    let ai_service = match AiService::new(openai_api_key, openrouter_api_key, morpheus_api_key, ollama_base_url, api_key).await {
        Ok(mut service) => {
            // Set the email service so AI can fetch real emails
            service.set_email_service(email_service.clone());
            Arc::new(service)
        },
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
        cache_service,
        config_service,
        ai_service,
        email_service,
        sync_service,
        account_service,
        sse_manager,
        event_bus,
        health_service: Some(health_service),
        config, // Pass the web::Data<Settings>
        imap_session_factory,
        connection_pool,
    })
}
