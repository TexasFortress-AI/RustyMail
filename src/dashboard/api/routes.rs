// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{web, Scope};
use super::handlers;
use super::accounts;
use super::sse;
use super::config;
use super::health;
use super::attachments;
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
        .route("/mcp/tools", web::get().to(handlers::list_mcp_tools))
        .route("/mcp/execute", web::post().to(handlers::execute_mcp_tool))
        // AI provider management endpoints
        .route("/ai/providers", web::get().to(handlers::get_ai_providers))
        .route("/ai/providers/set", web::post().to(handlers::set_ai_provider))
        // AI model management endpoints
        .route("/ai/models", web::get().to(handlers::get_ai_models))
        .route("/ai/models/set", web::post().to(handlers::set_ai_model))
        // Email sync endpoints
        .route("/sync/trigger", web::post().to(handlers::trigger_email_sync))
        .route("/sync/status", web::get().to(handlers::get_sync_status))
        // Email cache endpoints
        .route("/folders", web::get().to(handlers::list_folders))
        .route("/emails", web::get().to(handlers::get_cached_emails))
        // SMTP email sending endpoint
        .route("/emails/send", web::post().to(handlers::send_email))
        // Email deletion endpoint
        .route("/emails/delete", web::post().to(handlers::delete_email))
        .route("/events", web::get().to(sse::sse_handler))
        // Account management endpoints
        .route("/accounts/auto-config", web::post().to(accounts::auto_configure))
        .route("/accounts", web::post().to(accounts::create_account))
        .route("/accounts", web::get().to(accounts::list_accounts))
        .route("/accounts/default", web::get().to(accounts::get_default_account))
        .route("/accounts/{id}", web::get().to(accounts::get_account))
        .route("/accounts/{id}", web::put().to(accounts::update_account))
        .route("/accounts/{id}", web::delete().to(accounts::delete_account))
        .route("/accounts/{id}/default", web::post().to(accounts::set_default_account))
        .route("/accounts/{id}/connection-status", web::get().to(accounts::get_connection_status))
        .route("/accounts/{id}/validate", web::post().to(accounts::validate_connection))
        // Subscription management endpoints
        .route("/events/types", web::get().to(handlers::get_available_event_types))
        .route("/clients/{client_id}/subscriptions", web::get().to(handlers::get_client_subscriptions))
        .route("/clients/{client_id}/subscriptions", web::put().to(handlers::update_client_subscriptions))
        .route("/clients/{client_id}/subscribe", web::post().to(handlers::subscribe_to_event))
        .route("/clients/{client_id}/unsubscribe", web::post().to(handlers::unsubscribe_from_event))
        // Attachment management endpoints
        .route("/attachments/list", web::get().to(attachments::list_attachments))
        .route("/attachments/{message_id}/zip", web::get().to(attachments::download_attachments_zip))
        .route("/attachments/{message_id}/inline/{content_id}", web::get().to(attachments::download_inline_attachment))
        .route("/attachments/{message_id}/{filename}", web::get().to(attachments::download_attachment))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    info!("Configuring dashboard routes (/api/dashboard)");
    cfg.service(configure_routes());

    // Add health check endpoints
    health::configure_health_routes(cfg);
}
