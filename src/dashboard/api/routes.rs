use actix_web::{web, Scope};
use super::handlers;
use super::sse;
use super::config;
use super::health;
use log::info;

pub fn configure_routes() -> Scope {
    web::scope("/api/dashboard")
        .route("/stats", web::get().to(handlers::get_dashboard_stats))
        .route("/clients", web::get().to(handlers::get_connected_clients))
        .route("/config", web::get().to(config::get_config))
        .route("/config/imap", web::put().to(config::update_imap))
        .route("/config/rest", web::put().to(config::update_rest))
        .route("/config/dashboard", web::put().to(config::update_dashboard))
        .route("/config/validate", web::get().to(config::validate_config))
        .route("/chatbot/query", web::post().to(handlers::query_chatbot))
        .route("/chatbot/stream", web::post().to(handlers::stream_chatbot))
        // AI provider management endpoints
        .route("/ai/providers", web::get().to(handlers::get_ai_providers))
        .route("/ai/providers/set", web::post().to(handlers::set_ai_provider))
        // AI model management endpoints
        .route("/ai/models", web::get().to(handlers::get_ai_models))
        .route("/ai/models/set", web::post().to(handlers::set_ai_model))
        .route("/events", web::get().to(sse::sse_handler))
        // Subscription management endpoints
        .route("/events/types", web::get().to(handlers::get_available_event_types))
        .route("/clients/{client_id}/subscriptions", web::get().to(handlers::get_client_subscriptions))
        .route("/clients/{client_id}/subscriptions", web::put().to(handlers::update_client_subscriptions))
        .route("/clients/{client_id}/subscribe", web::post().to(handlers::subscribe_to_event))
        .route("/clients/{client_id}/unsubscribe", web::post().to(handlers::unsubscribe_from_event))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    info!("Configuring dashboard routes (/api/dashboard)");
    cfg.service(configure_routes());

    // Add health check endpoints
    health::configure_health_routes(cfg);
}
