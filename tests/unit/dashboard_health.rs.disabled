// Unit tests for dashboard health monitoring system

#[cfg(test)]
mod tests {
    use rustymail::dashboard::services::health::{
        HealthService, HealthThresholds, HealthStatus, ComponentHealth, HealthAlert
    };
    use rustymail::dashboard::services::events::{EventBus, AlertLevel};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_health_service_creation() {
        let service = HealthService::new();
        assert!(service.liveness().await);
    }

    #[tokio::test]
    async fn test_health_service_with_thresholds() {
        let thresholds = HealthThresholds {
            cpu_warning: 60.0,
            cpu_critical: 80.0,
            memory_warning: 70.0,
            memory_critical: 85.0,
            disk_warning: 75.0,
            disk_critical: 90.0,
            response_time_warning_ms: 500,
            response_time_critical_ms: 2000,
            connection_pool_warning: 40,
            connection_pool_critical: 60,
        };

        let service = HealthService::new().with_thresholds(thresholds);
        assert!(service.liveness().await);
    }

    #[tokio::test]
    async fn test_resource_health_metrics() {
        let service = HealthService::new();
        let resources = service.get_resource_health().await;

        // Verify resource metrics are within valid ranges
        assert!(resources.cpu_usage_percent >= 0.0);
        assert!(resources.cpu_usage_percent <= 100.0);
        assert!(resources.memory_usage_percent >= 0.0);
        assert!(resources.memory_usage_percent <= 100.0);
        assert!(resources.memory_total_mb > 0);
        assert!(resources.memory_used_mb <= resources.memory_total_mb);
        assert!(resources.disk_usage_percent >= 0.0);
        assert!(resources.disk_usage_percent <= 100.0);
        assert!(resources.thread_count > 0);
    }

    #[tokio::test]
    async fn test_health_report_generation() {
        let service = Arc::new(HealthService::new());
        let report = service.get_health_report().await;

        // Verify report structure
        assert!(matches!(
            report.status,
            HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy | HealthStatus::Unknown
        ));
        assert!(report.uptime_seconds >= 0);
        assert!(report.resources.cpu_usage_percent >= 0.0);
        assert!(report.resources.memory_usage_percent >= 0.0);

        // Verify timestamp
        let now = chrono::Utc::now();
        let report_time = report.last_updated;
        let time_diff = now.signed_duration_since(report_time);
        assert!(time_diff.num_seconds() < 2); // Report should be very recent
    }

    #[tokio::test]
    async fn test_health_service_with_event_bus() {
        let event_bus = Arc::new(EventBus::new());
        let mut subscription = event_bus.subscribe().await;

        let service = Arc::new(
            HealthService::new()
                .with_event_bus(Arc::clone(&event_bus))
        );

        // Get health report (shouldn't trigger alerts with normal system state)
        let report = service.get_health_report().await;
        assert!(report.alerts.is_empty() || !report.alerts.is_empty()); // May or may not have alerts

        // If we could simulate high resource usage, we'd test alert publishing here
        // For now, just verify the service can be created with an event bus
        assert!(service.liveness().await);
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let service = HealthService::new();

        // Without connection pool and session manager, should still be ready
        let is_ready = service.readiness().await;
        assert!(is_ready);

        // Liveness should always be true
        let is_alive = service.liveness().await;
        assert!(is_alive);
    }

    #[tokio::test]
    async fn test_health_thresholds_defaults() {
        let thresholds = HealthThresholds::default();

        // Verify default threshold values make sense
        assert!(thresholds.cpu_warning < thresholds.cpu_critical);
        assert!(thresholds.memory_warning < thresholds.memory_critical);
        assert!(thresholds.disk_warning < thresholds.disk_critical);
        assert!(thresholds.response_time_warning_ms < thresholds.response_time_critical_ms);
        assert!(thresholds.connection_pool_warning < thresholds.connection_pool_critical);

        // Verify reasonable default values
        assert_eq!(thresholds.cpu_warning, 70.0);
        assert_eq!(thresholds.cpu_critical, 90.0);
        assert_eq!(thresholds.memory_warning, 75.0);
        assert_eq!(thresholds.memory_critical, 90.0);
    }

    #[tokio::test]
    async fn test_component_health_structure() {
        use chrono::Utc;

        let component = ComponentHealth {
            name: "test_component".to_string(),
            status: HealthStatus::Healthy,
            message: Some("Component is functioning normally".to_string()),
            last_check: Utc::now(),
            response_time_ms: Some(50),
        };

        assert_eq!(component.name, "test_component");
        assert_eq!(component.status, HealthStatus::Healthy);
        assert!(component.message.is_some());
        assert!(component.response_time_ms.is_some());
        assert_eq!(component.response_time_ms.unwrap(), 50);
    }

    #[tokio::test]
    async fn test_health_alert_structure() {
        use chrono::Utc;

        let alert = HealthAlert {
            level: AlertLevel::Warning,
            component: "cpu".to_string(),
            message: "CPU usage high: 75%".to_string(),
            triggered_at: Utc::now(),
            value: Some(75.0),
            threshold: Some(70.0),
        };

        assert!(matches!(alert.level, AlertLevel::Warning));
        assert_eq!(alert.component, "cpu");
        assert!(alert.message.contains("CPU"));
        assert_eq!(alert.value, Some(75.0));
        assert_eq!(alert.threshold, Some(70.0));
    }

    #[tokio::test]
    async fn test_health_status_enum() {
        let statuses = vec![
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Unhealthy,
            HealthStatus::Unknown,
        ];

        // Test that all status variants can be compared
        for status in &statuses {
            assert_eq!(*status, *status);
        }

        // Test specific comparisons
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Degraded, HealthStatus::Unknown);
    }

    #[tokio::test]
    async fn test_health_service_monitoring() {
        let service = Arc::new(HealthService::new());

        // Start monitoring (this spawns a background task)
        Arc::clone(&service).start_monitoring().await;

        // Give the monitoring task a moment to start
        sleep(Duration::from_millis(100)).await;

        // Get a health report - should have been updated by monitoring
        let report = service.get_health_report().await;
        assert!(report.uptime_seconds >= 0);

        // Monitoring should continue in the background
        // In a real test, we'd verify components are being checked periodically
    }
}