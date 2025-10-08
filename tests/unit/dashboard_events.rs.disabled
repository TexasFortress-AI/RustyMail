// Unit tests for dashboard event broadcasting system

#[cfg(test)]
mod tests {
    use rustymail::dashboard::services::{EventBus, DashboardEvent};
    use rustymail::dashboard::services::events::{AlertLevel, ConfigSection};
    use rustymail::dashboard::api::models::{ClientType, ClientStatus, DashboardStats, SystemHealth};
    use std::collections::HashMap;
    use chrono::Utc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_event_bus_subscribe_and_publish() {
        let event_bus = EventBus::new();

        // Subscribe to events
        let mut subscription = event_bus.subscribe().await;

        // Publish an event
        event_bus.publish_system_alert(
            AlertLevel::Info,
            "Test alert".to_string(),
            None,
        ).await;

        // Receive the event
        let event = subscription.recv().await;
        assert!(event.is_some());

        match event.unwrap() {
            DashboardEvent::SystemAlert { level, message, .. } => {
                assert!(matches!(level, AlertLevel::Info));
                assert_eq!(message, "Test alert");
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let event_bus = EventBus::new();

        // Create multiple subscribers
        let mut sub1 = event_bus.subscribe().await;
        let mut sub2 = event_bus.subscribe().await;

        // Verify subscriber count
        assert_eq!(event_bus.subscriber_count().await, 2);

        // Publish an event
        event_bus.publish_metrics_updated(DashboardStats {
            active_dashboard_sse_clients: 5,
            requests_per_minute: 120.0,
            average_response_time_ms: 45.5,
            system_health: SystemHealth {
                status: rustymail::dashboard::api::models::SystemStatus::Healthy,
                memory_usage: 60.0,
                cpu_usage: 45.0,
            },
            last_updated: Utc::now().to_rfc3339(),
        }).await;

        // Both subscribers should receive the event
        let event1 = sub1.recv().await;
        let event2 = sub2.recv().await;

        assert!(event1.is_some());
        assert!(event2.is_some());

        // Both should receive the same type of event
        match (event1.unwrap(), event2.unwrap()) {
            (
                DashboardEvent::MetricsUpdated { stats: stats1, .. },
                DashboardEvent::MetricsUpdated { stats: stats2, .. }
            ) => {
                assert_eq!(stats1.active_dashboard_sse_clients, stats2.active_dashboard_sse_clients);
                assert_eq!(stats1.requests_per_minute, stats2.requests_per_minute);
            }
            _ => panic!("Unexpected event types"),
        }
    }

    #[tokio::test]
    async fn test_event_history() {
        let event_bus = EventBus::new();

        // Publish multiple events
        for i in 0..5 {
            event_bus.publish_system_alert(
                AlertLevel::Info,
                format!("Alert {}", i),
                None,
            ).await;
        }

        // Small delay to ensure events are processed
        sleep(Duration::from_millis(10)).await;

        // Get history
        let history = event_bus.get_history(3).await;
        assert_eq!(history.len(), 3);

        // Verify we got the most recent events (2, 3, 4)
        match &history[2] {
            DashboardEvent::SystemAlert { message, .. } => {
                assert_eq!(message, "Alert 4");
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_client_events() {
        let event_bus = EventBus::new();
        let mut subscription = event_bus.subscribe().await;

        // Publish client connected event
        event_bus.publish_client_connected(
            "test-client-123".to_string(),
            ClientType::Api,
            Some("192.168.1.1".to_string()),
            Some("Test Browser".to_string()),
        ).await;

        // Receive and verify the event
        let event = subscription.recv().await.unwrap();
        match event {
            DashboardEvent::ClientConnected {
                client_id,
                client_type,
                ip_address,
                user_agent,
                ..
            } => {
                assert_eq!(client_id, "test-client-123");
                assert!(matches!(client_type, ClientType::Api));
                assert_eq!(ip_address, Some("192.168.1.1".to_string()));
                assert_eq!(user_agent, Some("Test Browser".to_string()));
            }
            _ => panic!("Unexpected event type"),
        }

        // Publish client disconnected event
        event_bus.publish_client_disconnected(
            "test-client-123".to_string(),
            Some("Session timeout".to_string()),
        ).await;

        // Receive and verify the disconnect event
        let event = subscription.recv().await.unwrap();
        match event {
            DashboardEvent::ClientDisconnected { client_id, reason, .. } => {
                assert_eq!(client_id, "test-client-123");
                assert_eq!(reason, Some("Session timeout".to_string()));
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_configuration_events() {
        let event_bus = EventBus::new();
        let mut subscription = event_bus.subscribe().await;

        // Publish configuration update event
        let mut changes = HashMap::new();
        changes.insert("host".to_string(), serde_json::json!("imap.example.com"));
        changes.insert("port".to_string(), serde_json::json!(993));

        event_bus.publish_configuration_updated(
            ConfigSection::Imap,
            changes.clone(),
        ).await;

        // Receive and verify the event
        let event = subscription.recv().await.unwrap();
        match event {
            DashboardEvent::ConfigurationUpdated { section, changes: event_changes, .. } => {
                assert!(matches!(section, ConfigSection::Imap));
                assert_eq!(event_changes.get("host"), changes.get("host"));
                assert_eq!(event_changes.get("port"), changes.get("port"));
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let event_bus = EventBus::new();

        // Subscribe
        let subscription = event_bus.subscribe().await;
        let sub_id = subscription.id().to_string();

        assert_eq!(event_bus.subscriber_count().await, 1);

        // Unsubscribe
        event_bus.unsubscribe(&sub_id).await;

        assert_eq!(event_bus.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let event_bus = EventBus::new();

        // Add some events
        for i in 0..3 {
            event_bus.publish_system_alert(
                AlertLevel::Info,
                format!("Alert {}", i),
                None,
            ).await;
        }

        sleep(Duration::from_millis(10)).await;

        // Verify history has events
        let history = event_bus.get_history(10).await;
        assert!(!history.is_empty());

        // Clear history
        event_bus.clear_history().await;

        // Verify history is empty
        let history = event_bus.get_history(10).await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_event_bus_cloning() {
        let event_bus = EventBus::new();
        let event_bus_clone = event_bus.clone();

        // Subscribe from original
        let mut sub1 = event_bus.subscribe().await;

        // Subscribe from clone
        let mut sub2 = event_bus_clone.subscribe().await;

        // Publish from original
        event_bus.publish_system_alert(
            AlertLevel::Info,
            "Test from original".to_string(),
            None,
        ).await;

        // Both should receive the event
        let event1 = sub1.recv().await;
        let event2 = sub2.recv().await;

        assert!(event1.is_some());
        assert!(event2.is_some());
    }
}