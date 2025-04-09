use actix_web::{web, Scope};
use super::handlers;
use super::sse;
use log::info;

pub fn configure_routes() -> Scope {
    web::scope("/api/dashboard")
        .route("/stats", web::get().to(handlers::get_dashboard_stats))
        .route("/clients", web::get().to(handlers::get_connected_clients))
        .route("/config", web::get().to(handlers::get_configuration))
        .route("/chatbot/query", web::post().to(handlers::query_chatbot))
        .route("/events", web::get().to(sse::sse_handler))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    info!("Configuring dashboard routes (/api/dashboard)");
    cfg.service(configure_routes());
}
