use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::Infallible;
use log::{debug, warn};
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use crate::dashboard::api::models::{ChatbotQuery, ServerConfig};
use crate::dashboard::api::sse::EventType;
use crate::dashboard::services::ai::provider_manager::ProviderConfig;
use actix_web_lab::sse::{self, Sse};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid;
use serde_json;

// Query parameters for client list endpoint
#[derive(Debug, Deserialize)]
pub struct ClientQueryParams {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub filter: Option<String>,
}

// Handler for getting dashboard statistics
pub async fn get_dashboard_stats(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/stats");
    
    let stats = state.metrics_service.get_current_stats().await;
    
    Ok(HttpResponse::Ok().json(stats))
}

// Handler for getting client list
pub async fn get_connected_clients(
    query: web::Query<ClientQueryParams>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/clients with query: {:?}", query);
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let filter = query.filter.as_deref();
    
    if page == 0 {
        return Err(ApiError::BadRequest("Page must be at least 1".to_string()));
    }
    
    if limit == 0 {
        return Err(ApiError::BadRequest("Limit must be at least 1".to_string()));
    }
    
    let clients = state.client_manager.get_clients(page, limit, filter).await;
    
    Ok(HttpResponse::Ok().json(clients))
}

// Handler for getting server configuration
pub async fn get_configuration(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/config");
    
    let config: ServerConfig = state.config_service.get_configuration().await;
    
    Ok(HttpResponse::Ok().json(config))
}

// Handler for chatbot queries
pub async fn query_chatbot(
    state: web::Data<DashboardState>,
    req: web::Json<ChatbotQuery>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/chatbot/query with body: {:?}", req);

    let response = state.ai_service.process_query(req.0)
        .await
        .map_err(|e| ApiError::InternalError(format!("AI service error: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

// Handler for streaming chatbot responses via SSE
pub async fn stream_chatbot(
    state: web::Data<DashboardState>,
    req: web::Json<ChatbotQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<sse::Event, Infallible>>>, ApiError> {
    debug!("Handling POST /api/dashboard/chatbot/stream with body: {:?}", req);

    let (tx, rx) = mpsc::channel(100);
    let ai_service = state.ai_service.clone();
    let query = req.into_inner();

    // Spawn task to process query and stream response
    tokio::spawn(async move {
        // First send a "start" event
        let start_event = sse::Data::new(serde_json::json!({
            "type": "start",
            "conversation_id": query.conversation_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        }).to_string())
            .event("chatbot");

        if tx.send(Ok(sse::Event::Data(start_event))).await.is_err() {
            return;
        }

        // Process the query
        match ai_service.process_query(query).await {
            Ok(response) => {
                // For now, send the full response at once
                // TODO: Implement actual token-by-token streaming when provider supports it
                let content_event = sse::Data::new(serde_json::json!({
                    "type": "content",
                    "text": response.text,
                    "conversation_id": response.conversation_id,
                    "email_data": response.email_data,
                    "followup_suggestions": response.followup_suggestions
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(content_event))).await;

                // Send completion event
                let complete_event = sse::Data::new(serde_json::json!({
                    "type": "complete"
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(complete_event))).await;
            }
            Err(e) => {
                // Send error event
                let error_event = sse::Data::new(serde_json::json!({
                    "type": "error",
                    "error": format!("AI service error: {}", e)
                }).to_string())
                    .event("chatbot");

                let _ = tx.send(Ok(sse::Event::Data(error_event))).await;
            }
        }
    });

    // Convert receiver to stream
    let stream = ReceiverStream::new(rx);

    Ok(Sse::from_stream(stream))
}

// Request/response structures for subscription management
#[derive(Debug, Deserialize)]
pub struct SubscriptionRequest {
    pub event_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub client_id: String,
    pub subscriptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub event_type: String,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub event_type: String,
}

// Path parameters for subscription endpoints
#[derive(Debug, Deserialize)]
pub struct ClientIdPath {
    pub client_id: String,
}

// Handler for getting client subscriptions
pub async fn get_client_subscriptions(
    path: web::Path<ClientIdPath>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/clients/{}/subscriptions", path.client_id);

    match state.sse_manager.get_client_subscriptions(&path.client_id).await {
        Some(subscriptions) => {
            let subscription_strings: Vec<String> = subscriptions
                .iter()
                .map(|et| et.to_string().to_string())
                .collect();

            let response = SubscriptionResponse {
                client_id: path.client_id.clone(),
                subscriptions: subscription_strings,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        None => Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for updating client subscriptions (PUT - replaces all)
pub async fn update_client_subscriptions(
    path: web::Path<ClientIdPath>,
    req: web::Json<SubscriptionRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling PUT /api/dashboard/clients/{}/subscriptions", path.client_id);

    // Convert string event types to EventType enum
    let mut event_types = HashSet::new();
    for event_str in &req.event_types {
        match EventType::from_string(event_str) {
            Some(event_type) => {
                event_types.insert(event_type);
            }
            None => {
                return Err(ApiError::BadRequest(format!("Invalid event type: {}", event_str)));
            }
        }
    }

    // Update subscriptions
    let success = state.sse_manager
        .update_client_subscriptions(&path.client_id, event_types.clone())
        .await;

    if success {
        let subscription_strings: Vec<String> = event_types
            .iter()
            .map(|et| et.to_string().to_string())
            .collect();

        let response = SubscriptionResponse {
            client_id: path.client_id.clone(),
            subscriptions: subscription_strings,
        };

        Ok(HttpResponse::Ok().json(response))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for subscribing to a single event type (POST)
pub async fn subscribe_to_event(
    path: web::Path<ClientIdPath>,
    req: web::Json<SubscribeRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/clients/{}/subscribe", path.client_id);

    // Convert string to EventType
    let event_type = EventType::from_string(&req.event_type)
        .ok_or_else(|| ApiError::BadRequest(format!("Invalid event type: {}", req.event_type)))?;

    // Subscribe client
    let success = state.sse_manager
        .subscribe_client_to_event(&path.client_id, event_type)
        .await;

    if success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Client {} subscribed to {}", path.client_id, req.event_type)
        })))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for unsubscribing from a single event type (POST)
pub async fn unsubscribe_from_event(
    path: web::Path<ClientIdPath>,
    req: web::Json<UnsubscribeRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/clients/{}/unsubscribe", path.client_id);

    // Convert string to EventType
    let event_type = EventType::from_string(&req.event_type)
        .ok_or_else(|| ApiError::BadRequest(format!("Invalid event type: {}", req.event_type)))?;

    // Unsubscribe client
    let success = state.sse_manager
        .unsubscribe_client_from_event(&path.client_id, &event_type)
        .await;

    if success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Client {} unsubscribed from {}", path.client_id, req.event_type)
        })))
    } else {
        Err(ApiError::NotFound("Client not found".to_string()))
    }
}

// Handler for listing available event types
pub async fn get_available_event_types() -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/events/types");

    let event_types = vec![
        EventType::Welcome.to_string(),
        EventType::StatsUpdate.to_string(),
        EventType::ClientConnected.to_string(),
        EventType::ClientDisconnected.to_string(),
        EventType::SystemAlert.to_string(),
        EventType::ConfigurationUpdated.to_string(),
        EventType::DashboardEvent.to_string(),
    ];

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "available_event_types": event_types,
        "description": {
            "welcome": "Welcome message sent when client connects",
            "stats_update": "Real-time dashboard statistics updates",
            "client_connected": "Notifications when new clients connect",
            "client_disconnected": "Notifications when clients disconnect",
            "system_alert": "System alerts and warnings",
            "configuration_updated": "Configuration change notifications",
            "dashboard_event": "Generic dashboard events"
        }
    })))
}

// AI Provider Management Handlers

#[derive(Debug, Deserialize)]
pub struct SetProviderRequest {
    pub provider_name: String,
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub current_provider: Option<String>,
    pub available_providers: Vec<ProviderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SetModelRequest {
    pub model_name: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub current_model: Option<String>,
    pub available_models: Vec<String>,
    pub provider: Option<String>,
}

// Handler for getting AI provider status and list
pub async fn get_ai_providers(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/providers");

    let providers = state.ai_service.list_providers().await;
    let current_provider = state.ai_service.get_current_provider_name().await;

    let response = ProvidersResponse {
        current_provider,
        available_providers: providers,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Handler for setting the current AI provider
pub async fn set_ai_provider(
    req: web::Json<SetProviderRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/providers/set with provider: {}", req.provider_name);

    // Set the current provider
    state.ai_service
        .set_current_provider(req.provider_name.clone())
        .await
        .map_err(|e| ApiError::BadRequest(e))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Successfully switched to provider: {}", req.provider_name),
        "current_provider": req.provider_name
    })))
}

// Handler for getting available models for current AI provider
pub async fn get_ai_models(
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling GET /api/dashboard/ai/models");

    let current_provider = state.ai_service.get_current_provider_name().await;
    let providers = state.ai_service.list_providers().await;

    // Get current model from the current provider
    let current_model = if let Some(ref provider_name) = current_provider {
        providers.iter()
            .find(|p| p.name == *provider_name)
            .map(|p| p.model.clone())
    } else {
        None
    };

    // Dynamically fetch available models from the current provider
    let available_models = if let Some(provider_name) = current_provider.as_deref() {
        match state.ai_service.get_available_models().await {
            Ok(models) => models,
            Err(e) => {
                warn!("Failed to fetch models from provider {}: {:?}", provider_name, e);
                // Fallback to empty list if API call fails
                vec![]
            }
        }
    } else {
        vec![]
    };

    let response = ModelsResponse {
        current_model,
        available_models,
        provider: current_provider,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Handler for setting the model for current AI provider
pub async fn set_ai_model(
    req: web::Json<SetModelRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/ai/models/set with model: {}", req.model_name);

    let current_provider = state.ai_service.get_current_provider_name().await;

    if let Some(provider_name) = current_provider {
        // Get current provider config
        let providers = state.ai_service.list_providers().await;
        if let Some(current_config) = providers.iter().find(|p| p.name == provider_name) {
            // Create updated config with new model
            let mut new_config = current_config.clone();
            new_config.model = req.model_name.clone();

            // Update the provider config
            state.ai_service
                .update_provider_config(&provider_name, new_config)
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to update model: {}", e)))?;

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": format!("Successfully set model to: {}", req.model_name),
                "model": req.model_name,
                "provider": provider_name
            })))
        } else {
            Err(ApiError::BadRequest("Current provider configuration not found".to_string()))
        }
    } else {
        Err(ApiError::BadRequest("No current provider set".to_string()))
    }
}
