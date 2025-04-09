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
pub use ai::{AiService, providers::{AiProvider, OpenAiAdapter, OpenRouterAdapter}};

use actix_web::web;
use std::sync::Arc;
use std::time::Duration;
use crate::config::Settings;
use log::{info, warn};
use crate::dashboard::api::SseManager;
use crate::imap::ImapClient;
use reqwest::Client;

// Define a mock provider for when no API key is found
use async_trait::async_trait;
use crate::dashboard::api::errors::ApiError;
use ai::providers::AiChatMessage;

#[derive(Debug)]
struct MockAiProvider;

#[async_trait]
impl AiProvider for MockAiProvider {
    async fn generate_response(&self, _messages: &[AiChatMessage]) -> Result<String, ApiError> {
        // This mock provider doesn't actually generate responses itself.
        // The AiService handles the mock logic when it detects mock_mode.
        // This implementation just needs to satisfy the trait.
        // We could return an error, but returning an empty string might be simpler
        // as the AiService will override it with a mock response anyway.
        Ok("".to_string())
    }
}

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
    
    // Create common http client for AI providers
    let http_client = Client::new();

    // Initialize AI Provider based on environment variables
    let (ai_provider, force_mock): (Arc<dyn AiProvider>, bool) = 
        match std::env::var("OPENROUTER_API_KEY") {
            Ok(key) if !key.is_empty() => {
                info!("Using OpenRouter AI provider.");
                (Arc::new(OpenRouterAdapter::new(key, http_client.clone())), false)
            }
            _ => match std::env::var("OPENAI_API_KEY") {
                Ok(key) if !key.is_empty() => {
                    info!("Using OpenAI AI provider.");
                    (Arc::new(OpenAiAdapter::new(key, http_client.clone())), false)
                }
                _ => {
                    warn!("No OPENROUTER_API_KEY or OPENAI_API_KEY found. AI service will use mock responses.");
                    // Use a Mock provider instance, AiService will handle mock generation
                    (Arc::new(MockAiProvider), true) 
                }
            },
        };

    // Create other services
    let metrics_service = Arc::new(MetricsService::new(Duration::from_secs(5)));
    let client_manager = Arc::new(ClientManager::new(Duration::from_secs(600)));
    let config_service = Arc::new(ConfigService::new());
    // Instantiate AiService with the chosen provider
    let ai_service = Arc::new(AiService::new(ai_provider, force_mock)); 

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
