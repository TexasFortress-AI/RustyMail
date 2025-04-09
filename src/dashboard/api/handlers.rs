use actix_web::{web, HttpResponse, Responder};
use std::sync::Arc;
use serde::Deserialize;
use log::{info, debug};
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use crate::dashboard::api::models::{ChatbotQuery, ChatbotResponse};

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
    debug!("Handling GET /api/dashboard/clients");
    
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
    
    let config = state.config_service.get_configuration().await;
    
    Ok(HttpResponse::Ok().json(config))
}

// Handler for setting active adapter
#[derive(Debug, Deserialize)]
pub struct SetAdapterRequest {
    pub adapter_id: String,
}

pub async fn set_active_adapter(
    req: web::Json<SetAdapterRequest>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    let adapter_id = &req.adapter_id;
    info!("Handling POST /api/dashboard/config/adapter with adapter_id: {}", adapter_id);
    
    let config = state.config_service.set_active_adapter(adapter_id)
        .await
        .map_err(|e| ApiError::BadRequest(e))?;
    
    Ok(HttpResponse::Ok().json(config))
}

// Handler for chatbot queries
pub async fn query_chatbot(
    req: web::Json<ChatbotQuery>,
    state: web::Data<DashboardState>,
) -> Result<impl Responder, ApiError> {
    debug!("Handling POST /api/dashboard/chatbot/query");
    
    let response = state.ai_service.process_query(req.0)
        .await
        .map_err(|e| ApiError::InternalError(e))?;
    
    Ok(HttpResponse::Ok().json(response))
}
