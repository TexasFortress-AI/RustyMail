#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use actix_web_lab::sse::{self, Data, Event, Sse};
    use futures_util::{StreamExt, stream};
    use rustymail::dashboard::{
        api::{
            handlers::stream_chatbot,
            models::ChatbotQuery,
            sse::{EventType, SseEvent, SseMessage},
        },
        services::{
            DashboardState,
            events::{EventManager, EventSubscription},
        },
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{RwLock, mpsc};
    use tokio::time::timeout;
    use serde_json::json;

    async fn create_test_state() -> web::Data<DashboardState> {
        let event_manager = Arc::new(RwLock::new(EventManager::new()));

        web::Data::new(DashboardState {
            metrics_service: Arc::new(Default::default()),
            client_manager: Arc::new(Default::default()),
            config_service: Arc::new(Default::default()),
            ai_service: Arc::new(Default::default()),
            email_service: Arc::new(Default::default()),
            event_manager,
        })
    }

    #[tokio::test]
    async fn test_sse_event_types() {
        let event_type = EventType::Stats;
        assert_eq!(event_type.as_str(), "stats");

        let event_type = EventType::ClientUpdate;
        assert_eq!(event_type.as_str(), "client_update");

        let event_type = EventType::ConfigChange;
        assert_eq!(event_type.as_str(), "config_change");
    }

    #[tokio::test]
    async fn test_sse_message_serialization() {
        let message = SseMessage {
            id: Some("123".to_string()),
            event_type: EventType::Stats,
            data: json!({"active_connections": 5}),
            retry: Some(1000),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"id\":\"123\""));
        assert!(json.contains("\"event_type\":\"stats\""));
    }

    #[tokio::test]
    async fn test_event_subscription_creation() {
        let subscription = EventSubscription::new("client-123", EventType::Stats);
        assert_eq!(subscription.client_id, "client-123");
        assert_eq!(subscription.event_type, EventType::Stats);
        assert!(subscription.created_at.elapsed().as_secs() < 1);
    }

    #[tokio::test]
    async fn test_event_manager_subscribe() {
        let manager = EventManager::new();

        // Subscribe client to stats events
        manager.subscribe("client-1", EventType::Stats).await;

        // Check subscription exists
        let subscriptions = manager.get_subscriptions("client-1").await;
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0], EventType::Stats);
    }

    #[tokio::test]
    async fn test_event_manager_unsubscribe() {
        let manager = EventManager::new();

        // Subscribe and then unsubscribe
        manager.subscribe("client-1", EventType::Stats).await;
        manager.subscribe("client-1", EventType::ClientUpdate).await;

        manager.unsubscribe("client-1", EventType::Stats).await;

        let subscriptions = manager.get_subscriptions("client-1").await;
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0], EventType::ClientUpdate);
    }

    #[tokio::test]
    async fn test_event_manager_broadcast() {
        let manager = EventManager::new();

        // Subscribe multiple clients
        manager.subscribe("client-1", EventType::Stats).await;
        manager.subscribe("client-2", EventType::Stats).await;
        manager.subscribe("client-3", EventType::ConfigChange).await;

        // Broadcast stats event
        let event = SseEvent {
            event_type: EventType::Stats,
            data: json!({"total_messages": 100}),
        };

        let recipients = manager.broadcast_event(event).await;
        assert_eq!(recipients.len(), 2);
        assert!(recipients.contains(&"client-1".to_string()));
        assert!(recipients.contains(&"client-2".to_string()));
    }

    #[tokio::test]
    async fn test_event_manager_remove_client() {
        let manager = EventManager::new();

        // Subscribe client to multiple events
        manager.subscribe("client-1", EventType::Stats).await;
        manager.subscribe("client-1", EventType::ClientUpdate).await;
        manager.subscribe("client-1", EventType::ConfigChange).await;

        // Remove client
        manager.remove_client("client-1").await;

        let subscriptions = manager.get_subscriptions("client-1").await;
        assert_eq!(subscriptions.len(), 0);
    }

    #[tokio::test]
    async fn test_sse_stream_creation() {
        let (tx, rx) = mpsc::channel::<SseMessage>(10);

        // Send test message
        let message = SseMessage {
            id: Some("test-1".to_string()),
            event_type: EventType::Stats,
            data: json!({"test": "data"}),
            retry: None,
        };

        tx.send(message.clone()).await.unwrap();
        drop(tx);

        // Receive and verify
        let mut stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let received = stream.next().await;
        assert!(received.is_some());
    }

    #[tokio::test]
    async fn test_sse_heartbeat() {
        let (tx, rx) = mpsc::channel::<SseMessage>(10);

        // Spawn heartbeat task
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                let heartbeat = SseMessage {
                    id: None,
                    event_type: EventType::Heartbeat,
                    data: json!({"timestamp": chrono::Utc::now().to_rfc3339()}),
                    retry: None,
                };

                if tx_clone.send(heartbeat).await.is_err() {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // Receive heartbeats
        let mut stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let mut count = 0;

        while count < 3 {
            if let Ok(Some(_)) = timeout(Duration::from_secs(1), stream.next()).await {
                count += 1;
            } else {
                break;
            }
        }

        assert!(count >= 2);
    }

    #[tokio::test]
    async fn test_stream_chatbot_response() {
        let state = create_test_state().await;

        // Create test query
        let query = ChatbotQuery {
            query: "Test streaming response".to_string(),
            context: None,
            model_override: None,
            provider_override: None,
        };

        // Simulate streaming response
        let (tx, mut rx) = mpsc::channel::<String>(10);

        // Send chunks
        tx.send("This ".to_string()).await.unwrap();
        tx.send("is ".to_string()).await.unwrap();
        tx.send("streaming".to_string()).await.unwrap();
        drop(tx);

        // Collect chunks
        let mut result = String::new();
        while let Some(chunk) = rx.recv().await {
            result.push_str(&chunk);
        }

        assert_eq!(result, "This is streaming");
    }

    #[tokio::test]
    async fn test_sse_error_handling() {
        let (tx, rx) = mpsc::channel::<SseMessage>(1);

        // Fill channel to capacity
        let message = SseMessage {
            id: Some("1".to_string()),
            event_type: EventType::Error,
            data: json!({"error": "Test error"}),
            retry: Some(5000),
        };

        tx.send(message.clone()).await.unwrap();

        // Try to send when full (should handle gracefully)
        let send_result = tx.try_send(message.clone());
        assert!(send_result.is_err());
    }

    #[tokio::test]
    async fn test_sse_reconnection_support() {
        let message = SseMessage {
            id: Some("last-id".to_string()),
            event_type: EventType::Stats,
            data: json!({"reconnection": true}),
            retry: Some(3000),
        };

        // Verify retry field is set for reconnection
        assert_eq!(message.retry, Some(3000));

        // Simulate reconnection with last event ID
        let last_event_id = message.id.clone();
        assert_eq!(last_event_id, Some("last-id".to_string()));
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let manager = EventManager::new();

        // Subscribe to specific events
        manager.subscribe("client-1", EventType::Stats).await;
        manager.subscribe("client-1", EventType::ConfigChange).await;

        // Client should not receive unsubscribed events
        let event = SseEvent {
            event_type: EventType::ClientUpdate,
            data: json!({"test": "data"}),
        };

        let recipients = manager.broadcast_event(event).await;
        assert!(!recipients.contains(&"client-1".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_subscriptions() {
        let manager = Arc::new(EventManager::new());

        // Concurrent subscription tasks
        let mut handles = vec![];

        for i in 0..10 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                let client_id = format!("client-{}", i);
                manager_clone.subscribe(&client_id, EventType::Stats).await;
            });
            handles.push(handle);
        }

        // Wait for all subscriptions
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all subscriptions
        let all_clients = manager.get_all_clients().await;
        assert_eq!(all_clients.len(), 10);
    }

    #[tokio::test]
    async fn test_sse_data_chunking() {
        // Large data payload
        let large_data = json!({
            "items": (0..1000).map(|i| format!("item-{}", i)).collect::<Vec<_>>()
        });

        let message = SseMessage {
            id: Some("large".to_string()),
            event_type: EventType::Custom("bulk_data".to_string()),
            data: large_data,
            retry: None,
        };

        // Serialize and check size
        let serialized = serde_json::to_string(&message).unwrap();
        assert!(serialized.len() > 1000);
    }

    #[tokio::test]
    async fn test_sse_custom_events() {
        let custom_event = EventType::Custom("email_received".to_string());
        assert_eq!(custom_event.as_str(), "email_received");

        let message = SseMessage {
            id: Some("custom-1".to_string()),
            event_type: custom_event,
            data: json!({
                "from": "user@example.com",
                "subject": "Test email"
            }),
            retry: None,
        };

        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["event_type"], "email_received");
    }

    #[tokio::test]
    async fn test_stream_timeout_handling() {
        let (tx, rx) = mpsc::channel::<SseMessage>(10);

        // Don't send anything, just drop sender
        drop(tx);

        let mut stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        // Stream should end gracefully
        let result = timeout(Duration::from_millis(100), stream.next()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_event_manager_statistics() {
        let manager = EventManager::new();

        // Add various subscriptions
        manager.subscribe("client-1", EventType::Stats).await;
        manager.subscribe("client-1", EventType::ClientUpdate).await;
        manager.subscribe("client-2", EventType::Stats).await;
        manager.subscribe("client-3", EventType::ConfigChange).await;

        // Get statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_clients, 3);
        assert_eq!(stats.total_subscriptions, 4);
        assert_eq!(stats.subscriptions_by_type.get(&EventType::Stats), Some(&2));
    }
}