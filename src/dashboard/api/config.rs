use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::dashboard::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use log::info;

#[derive(Debug, Deserialize)]
pub struct UpdateImapConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRestConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDashboardConfig {
    pub enabled: bool,
    pub port: u16,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfigUpdateResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CurrentConfig {
    pub imap: ImapConfigInfo,
    pub rest: Option<RestConfigInfo>,
    pub dashboard: Option<DashboardConfigInfo>,
}

#[derive(Debug, Serialize)]
pub struct ImapConfigInfo {
    pub host: String,
    pub port: u16,
    pub user: String,
}

#[derive(Debug, Serialize)]
pub struct RestConfigInfo {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct DashboardConfigInfo {
    pub enabled: bool,
    pub port: u16,
    pub path: Option<String>,
}

pub async fn get_config(state: web::Data<DashboardState>) -> Result<HttpResponse, ApiError> {
    let settings = state.config_service.get_settings().await;

    let config = CurrentConfig {
        imap: ImapConfigInfo {
            host: settings.imap_host.clone(),
            port: settings.imap_port,
            user: settings.imap_user.clone(),
        },
        rest: settings.rest.map(|r| RestConfigInfo {
            enabled: r.enabled,
            host: r.host,
            port: r.port,
        }),
        dashboard: settings.dashboard.map(|d| DashboardConfigInfo {
            enabled: d.enabled,
            port: d.port,
            path: d.path,
        }),
    };

    Ok(HttpResponse::Ok().json(config))
}

pub async fn update_imap(
    state: web::Data<DashboardState>,
    config: web::Json<UpdateImapConfig>,
) -> Result<HttpResponse, ApiError> {
    info!("Updating IMAP configuration");

    match state.config_service.update_imap_config(
        config.host.clone(),
        config.port,
        config.user.clone(),
        config.pass.clone(),
    ).await {
        Ok(()) => {
            info!("IMAP configuration updated successfully");
            Ok(HttpResponse::Ok().json(ConfigUpdateResponse {
                success: true,
                message: "IMAP configuration updated successfully".to_string(),
            }))
        }
        Err(e) => {
            Ok(HttpResponse::BadRequest().json(ConfigUpdateResponse {
                success: false,
                message: e,
            }))
        }
    }
}

pub async fn update_rest(
    state: web::Data<DashboardState>,
    config: web::Json<UpdateRestConfig>,
) -> Result<HttpResponse, ApiError> {
    info!("Updating REST configuration");

    match state.config_service.update_rest_config(
        config.enabled,
        config.host.clone(),
        config.port,
    ).await {
        Ok(()) => {
            info!("REST configuration updated successfully");
            Ok(HttpResponse::Ok().json(ConfigUpdateResponse {
                success: true,
                message: "REST configuration updated successfully".to_string(),
            }))
        }
        Err(e) => {
            Ok(HttpResponse::BadRequest().json(ConfigUpdateResponse {
                success: false,
                message: e,
            }))
        }
    }
}

pub async fn update_dashboard(
    state: web::Data<DashboardState>,
    config: web::Json<UpdateDashboardConfig>,
) -> Result<HttpResponse, ApiError> {
    info!("Updating dashboard configuration");

    match state.config_service.update_dashboard_config(
        config.enabled,
        config.port,
        config.path.clone(),
    ).await {
        Ok(()) => {
            info!("Dashboard configuration updated successfully");
            Ok(HttpResponse::Ok().json(ConfigUpdateResponse {
                success: true,
                message: "Dashboard configuration updated successfully".to_string(),
            }))
        }
        Err(e) => {
            Ok(HttpResponse::BadRequest().json(ConfigUpdateResponse {
                success: false,
                message: e,
            }))
        }
    }
}

pub async fn validate_config(state: web::Data<DashboardState>) -> Result<HttpResponse, ApiError> {
    let settings = state.config_service.get_settings().await;

    match state.config_service.validate_config(&settings).await {
        Ok(()) => {
            Ok(HttpResponse::Ok().json(ConfigUpdateResponse {
                success: true,
                message: "Configuration is valid".to_string(),
            }))
        }
        Err(errors) => {
            Ok(HttpResponse::BadRequest().json(ConfigUpdateResponse {
                success: false,
                message: errors.join(", "),
            }))
        }
    }
}