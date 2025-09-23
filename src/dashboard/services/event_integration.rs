// Event Bus Integration for Dashboard Services
//
// This module provides helper functions to integrate the event bus
// with various dashboard services for automatic event publishing.

use std::sync::Arc;
use crate::dashboard::services::{
    EventBus, DashboardEvent, DashboardState,
    ClientManager, MetricsService, ConfigService,
};
use crate::dashboard::services::events::{AlertLevel, ConfigSection};
use crate::dashboard::api::models::{ClientType, ClientStatus};
use tokio::time::{interval, Duration};
use log::{info, debug};
use std::collections::HashMap;

/// Start all event publishers for dashboard services
pub async fn start_event_publishers(dashboard_state: Arc<DashboardState>) {
    info!("Starting event publishers for dashboard services");

    // Start metrics event publisher
    start_metrics_publisher(
        Arc::clone(&dashboard_state.metrics_service),
        Arc::clone(&dashboard_state.event_bus),
    ).await;

    // Start SSE event bus listener
    dashboard_state.sse_manager.start_event_bus_listener().await;

    // Start system health monitor
    start_health_monitor(Arc::clone(&dashboard_state)).await;

    info!("All event publishers started");
}

/// Start periodic metrics publishing
async fn start_metrics_publisher(metrics_service: Arc<MetricsService>, event_bus: Arc<EventBus>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Get current stats
            let stats = metrics_service.get_current_stats().await;

            // Publish metrics updated event
            event_bus.publish_metrics_updated(stats).await;

            debug!("Published metrics update event");
        }
    });

    info!("Started metrics event publisher");
}

/// Start system health monitoring and alerting
async fn start_health_monitor(dashboard_state: Arc<DashboardState>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(30));
        let mut last_health_status = true;

        loop {
            interval.tick().await;

            // Check system health
            let stats = dashboard_state.metrics_service.get_current_stats().await;
            let mut issues = Vec::new();
            let mut healthy = true;

            // Check CPU usage
            if stats.system_health.cpu_usage > 90.0 {
                issues.push("High CPU usage detected".to_string());
                healthy = false;

                dashboard_state.event_bus.publish_system_alert(
                    AlertLevel::Warning,
                    format!("CPU usage is at {:.1}%", stats.system_health.cpu_usage),
                    None,
                ).await;
            }

            // Check memory usage
            if stats.system_health.memory_usage > 85.0 {
                issues.push("High memory usage detected".to_string());
                healthy = false;

                dashboard_state.event_bus.publish_system_alert(
                    AlertLevel::Warning,
                    format!("Memory usage is at {:.1}%", stats.system_health.memory_usage),
                    None,
                ).await;
            }

            // Check connection pool health
            let pool_stats = dashboard_state.connection_pool.stats().await;
            if pool_stats.acquire_timeouts > 10 {
                issues.push("Connection pool experiencing timeouts".to_string());
                healthy = false;

                dashboard_state.event_bus.publish_system_alert(
                    AlertLevel::Error,
                    format!("Connection pool has {} acquire timeouts", pool_stats.acquire_timeouts),
                    None,
                ).await;
            }

            // Publish health change event if status changed
            if healthy != last_health_status {
                dashboard_state.event_bus.publish(DashboardEvent::SystemHealthChanged {
                    healthy,
                    issues: issues.clone(),
                    timestamp: chrono::Utc::now(),
                }).await;

                last_health_status = healthy;
            }

            debug!("Health monitor check completed - healthy: {}, issues: {:?}", healthy, issues);
        }
    });

    info!("Started system health monitor");
}

/// Wrapper for ClientManager to publish events
pub struct EventedClientManager {
    inner: Arc<ClientManager>,
    event_bus: Arc<EventBus>,
}

impl EventedClientManager {
    pub fn new(client_manager: Arc<ClientManager>, event_bus: Arc<EventBus>) -> Self {
        Self {
            inner: client_manager,
            event_bus,
        }
    }

    pub async fn register_client(
        &self,
        client_type: ClientType,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> String {
        let client_id = self.inner.register_client(
            client_type.clone(),
            ip_address.clone(),
            user_agent.clone(),
        ).await;

        // Publish client connected event
        self.event_bus.publish_client_connected(
            client_id.clone(),
            client_type,
            ip_address,
            user_agent,
        ).await;

        client_id
    }

    pub async fn remove_client(&self, client_id: &str) {
        self.inner.remove_client(client_id).await;

        // Publish client disconnected event
        self.event_bus.publish_client_disconnected(
            client_id.to_string(),
            Some("Client removed".to_string()),
        ).await;
    }

    pub async fn update_client_status(&self, client_id: &str, new_status: ClientStatus) {
        // Get current status before update
        let clients = self.inner.get_clients(1, 1000, None).await;
        let old_status = clients.clients.iter()
            .find(|c| c.id == client_id)
            .map(|c| c.status.clone())
            .unwrap_or(ClientStatus::Active); // Default to Active if not found

        self.inner.update_client_status(client_id, new_status.clone()).await;

        // Publish status change event
        self.event_bus.publish(DashboardEvent::ClientStatusChanged {
            client_id: client_id.to_string(),
            old_status,
            new_status,
            timestamp: chrono::Utc::now(),
        }).await;
    }
}

/// Wrapper for ConfigService to publish events
pub struct EventedConfigService {
    inner: Arc<ConfigService>,
    event_bus: Arc<EventBus>,
}

impl EventedConfigService {
    pub fn new(config_service: Arc<ConfigService>, event_bus: Arc<EventBus>) -> Self {
        Self {
            inner: config_service,
            event_bus,
        }
    }

    pub async fn update_imap_config(
        &self,
        host: String,
        port: u16,
        user: String,
        pass: String,
    ) -> Result<(), String> {
        let result = self.inner.update_imap_config(host.clone(), port, user.clone(), pass).await;

        match &result {
            Ok(_) => {
                let mut changes = HashMap::new();
                changes.insert("host".to_string(), serde_json::json!(host));
                changes.insert("port".to_string(), serde_json::json!(port));
                changes.insert("user".to_string(), serde_json::json!(user));

                self.event_bus.publish_configuration_updated(
                    ConfigSection::Imap,
                    changes,
                ).await;
            }
            Err(e) => {
                self.event_bus.publish(DashboardEvent::ConfigurationError {
                    section: ConfigSection::Imap,
                    error: e.clone(),
                    timestamp: chrono::Utc::now(),
                }).await;
            }
        }

        result
    }

    pub async fn update_rest_config(
        &self,
        enabled: bool,
        host: String,
        port: u16,
    ) -> Result<(), String> {
        let result = self.inner.update_rest_config(enabled, host.clone(), port).await;

        match &result {
            Ok(_) => {
                let mut changes = HashMap::new();
                changes.insert("enabled".to_string(), serde_json::json!(enabled));
                changes.insert("host".to_string(), serde_json::json!(host));
                changes.insert("port".to_string(), serde_json::json!(port));

                self.event_bus.publish_configuration_updated(
                    ConfigSection::Rest,
                    changes,
                ).await;
            }
            Err(e) => {
                self.event_bus.publish(DashboardEvent::ConfigurationError {
                    section: ConfigSection::Rest,
                    error: e.clone(),
                    timestamp: chrono::Utc::now(),
                }).await;
            }
        }

        result
    }

    pub async fn update_dashboard_config(
        &self,
        enabled: bool,
        port: u16,
        path: Option<String>,
    ) -> Result<(), String> {
        let result = self.inner.update_dashboard_config(enabled, port, path.clone()).await;

        match &result {
            Ok(_) => {
                let mut changes = HashMap::new();
                changes.insert("enabled".to_string(), serde_json::json!(enabled));
                changes.insert("port".to_string(), serde_json::json!(port));
                if let Some(p) = &path {
                    changes.insert("path".to_string(), serde_json::json!(p));
                }

                self.event_bus.publish_configuration_updated(
                    ConfigSection::Dashboard,
                    changes,
                ).await;
            }
            Err(e) => {
                self.event_bus.publish(DashboardEvent::ConfigurationError {
                    section: ConfigSection::Dashboard,
                    error: e.clone(),
                    timestamp: chrono::Utc::now(),
                }).await;
            }
        }

        result
    }
}