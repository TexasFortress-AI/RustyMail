use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use log::debug;
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use crate::dashboard::api::models::{ChatbotQuery, ServerConfig};

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
