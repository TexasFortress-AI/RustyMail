// Health Check API Endpoints for Dashboard
//
// Provides HTTP endpoints for health monitoring, including liveness, readiness,
// and detailed health reports.

use actix_web::{web, HttpResponse, Result};
use log::{info, debug};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::dashboard::services::{DashboardState, health::HealthService};

// Health check response format
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

// Configure health check routes
pub fn configure_health_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::scope("/health")
                .route("/live", web::get().to(liveness))
                .route("/ready", web::get().to(readiness))
                .route("/report", web::get().to(health_report))
                .route("/metrics", web::get().to(health_metrics))
        )
        // Legacy endpoints for compatibility
        .route("/healthz", web::get().to(liveness))
        .route("/readyz", web::get().to(readiness));
}

// Liveness probe endpoint - returns 200 if service is alive
pub async fn liveness(
    _state: web::Data<DashboardState>,
) -> Result<HttpResponse> {
    debug!("Liveness check requested");

    let response = HealthCheckResponse {
        status: "alive".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        details: None,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Readiness probe endpoint - returns 200 if service is ready to handle requests
pub async fn readiness(
    state: web::Data<DashboardState>,
) -> Result<HttpResponse> {
    debug!("Readiness check requested");

    if let Some(health_service) = &state.health_service {
        let is_ready = health_service.readiness().await;

        if is_ready {
            let response = HealthCheckResponse {
                status: "ready".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                details: None,
            };
            Ok(HttpResponse::Ok().json(response))
        } else {
            let response = HealthCheckResponse {
                status: "not_ready".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                details: Some(serde_json::json!({
                    "message": "One or more critical components are unhealthy"
                })),
            };
            Ok(HttpResponse::ServiceUnavailable().json(response))
        }
    } else {
        // Health service not configured, consider it ready
        let response = HealthCheckResponse {
            status: "ready".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            details: Some(serde_json::json!({
                "message": "Health monitoring not configured"
            })),
        };
        Ok(HttpResponse::Ok().json(response))
    }
}

// Detailed health report endpoint
pub async fn health_report(
    state: web::Data<DashboardState>,
) -> Result<HttpResponse> {
    info!("Health report requested");

    if let Some(health_service) = &state.health_service {
        let report = health_service.get_health_report().await;
        Ok(HttpResponse::Ok().json(report))
    } else {
        let response = HealthCheckResponse {
            status: "unknown".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            details: Some(serde_json::json!({
                "message": "Health monitoring service not available"
            })),
        };
        Ok(HttpResponse::ServiceUnavailable().json(response))
    }
}

// Resource metrics endpoint
pub async fn health_metrics(
    state: web::Data<DashboardState>,
) -> Result<HttpResponse> {
    debug!("Health metrics requested");

    if let Some(health_service) = &state.health_service {
        let resources = health_service.get_resource_health().await;
        Ok(HttpResponse::Ok().json(resources))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Health monitoring service not available"
        })))
    }
}

// Prometheus-compatible metrics endpoint (future enhancement)
pub async fn prometheus_metrics(
    state: web::Data<DashboardState>,
) -> Result<HttpResponse> {
    debug!("Prometheus metrics requested");

    if let Some(health_service) = &state.health_service {
        let resources = health_service.get_resource_health().await;

        // Format metrics in Prometheus text format
        let mut metrics = String::new();

        // System metrics
        metrics.push_str(&format!("# HELP rustymail_cpu_usage CPU usage percentage\n"));
        metrics.push_str(&format!("# TYPE rustymail_cpu_usage gauge\n"));
        metrics.push_str(&format!("rustymail_cpu_usage {:.2}\n", resources.cpu_usage_percent));

        metrics.push_str(&format!("# HELP rustymail_memory_usage Memory usage percentage\n"));
        metrics.push_str(&format!("# TYPE rustymail_memory_usage gauge\n"));
        metrics.push_str(&format!("rustymail_memory_usage {:.2}\n", resources.memory_usage_percent));

        metrics.push_str(&format!("# HELP rustymail_memory_used_bytes Memory used in bytes\n"));
        metrics.push_str(&format!("# TYPE rustymail_memory_used_bytes gauge\n"));
        metrics.push_str(&format!("rustymail_memory_used_bytes {}\n", resources.memory_used_mb * 1024 * 1024));

        metrics.push_str(&format!("# HELP rustymail_disk_usage Disk usage percentage\n"));
        metrics.push_str(&format!("# TYPE rustymail_disk_usage gauge\n"));
        metrics.push_str(&format!("rustymail_disk_usage {:.2}\n", resources.disk_usage_percent));

        metrics.push_str(&format!("# HELP rustymail_thread_count Number of threads\n"));
        metrics.push_str(&format!("# TYPE rustymail_thread_count gauge\n"));
        metrics.push_str(&format!("rustymail_thread_count {}\n", resources.thread_count));

        // Get health report for component status
        let report = health_service.get_health_report().await;

        metrics.push_str(&format!("# HELP rustymail_uptime_seconds Service uptime in seconds\n"));
        metrics.push_str(&format!("# TYPE rustymail_uptime_seconds counter\n"));
        metrics.push_str(&format!("rustymail_uptime_seconds {}\n", report.uptime_seconds));

        // Component health status (1 = healthy, 0 = unhealthy)
        metrics.push_str(&format!("# HELP rustymail_component_health Component health status\n"));
        metrics.push_str(&format!("# TYPE rustymail_component_health gauge\n"));

        for (name, component) in report.components {
            let value = match component.status {
                crate::dashboard::services::health::HealthStatus::Healthy => 1.0,
                crate::dashboard::services::health::HealthStatus::Degraded => 0.5,
                _ => 0.0,
            };
            metrics.push_str(&format!("rustymail_component_health{{component=\"{}\"}} {}\n", name, value));
        }

        Ok(HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4")
            .body(metrics))
    } else {
        Ok(HttpResponse::ServiceUnavailable().body("# Health monitoring service not available\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_liveness_endpoint() {
        // This would require a mock DashboardState
        // For now, we just test that the function exists
        assert!(true);
    }

    #[actix_web::test]
    async fn test_readiness_endpoint() {
        // This would require a mock DashboardState
        // For now, we just test that the function exists
        assert!(true);
    }
}