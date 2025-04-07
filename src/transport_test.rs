#[cfg(test)]
mod tests {
    use crate::transport::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock transport implementation for testing
    #[derive(Clone, Default)]
    struct MockTransport {
        is_connected: Arc<Mutex<bool>>,
        messages: Arc<Mutex<Vec<Message>>>,
        message_to_receive: Arc<Mutex<Option<Message>>>,
    }

    impl MockTransport {
        fn set_message_to_receive(&self, msg: Option<Message>) {
            *self.message_to_receive.blocking_lock() = msg;
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&self, message: Message) -> Result<(), TransportError> {
            if !*self.is_connected.lock().await {
                return Err(TransportError::ConnectionError("Not connected".to_string()));
            }
            self.messages.lock().await.push(message);
            Ok(())
        }

        async fn receive(&self) -> Result<Message, TransportError> {
            if !*self.is_connected.lock().await {
                return Err(TransportError::ConnectionError("Not connected".to_string()));
            }
            let mut msg_opt = self.message_to_receive.lock().await;
            msg_opt.take()
                .ok_or_else(|| TransportError::ReceiveError("No messages".to_string()))
        }

        async fn close(&self) -> Result<(), TransportError> {
            *self.is_connected.lock().await = false;
            Ok(())
        }

        async fn is_connected(&self) -> bool {
            *self.is_connected.lock().await
        }
    }

    #[tokio::test]
    async fn test_message_creation() {
        let request = Message::new_request(
            "test_id".to_string(),
            json!({ "param": "value" }),
        );
        assert_eq!(request.id, Some("test_id".to_string()));
        assert!(matches!(request.kind, MessageKind::Request));

        let response = Message::new_response(
            "test_id".to_string(),
            json!({ "result": "success" }),
        );
        assert_eq!(response.id, Some("test_id".to_string()));
        assert!(matches!(response.kind, MessageKind::Response));

        let notification = Message::new_notification(
            "test_event".to_string(),
            json!({ "data": 123 }),
        );
        assert_eq!(notification.id, None);
        assert!(matches!(notification.kind, MessageKind::Notification));

        let error = Message::new_error(
            Some("req_id".to_string()),
            -32000,
            "Test error".to_string(),
            Some(json!({ "details": "info" })),
        );
        assert_eq!(error.id, Some("req_id".to_string()));
        assert!(matches!(error.kind, MessageKind::Error));
    }

    #[tokio::test]
    async fn test_mock_transport_send_receive() {
        let transport = MockTransport::default();
        *transport.is_connected.lock().await = true;

        let msg = Message::new_request("1".into(), "test".into(), json!({}));
        transport.set_message_to_receive(Some(msg.clone()));

        let send_result = transport.send(msg.clone()).await;
        assert!(send_result.is_ok());
        assert_eq!(transport.messages.lock().await.len(), 1);
        assert_eq!(transport.messages.lock().await[0], msg);

        let received = transport.receive().await.unwrap();
        assert_eq!(received, msg);
        assert!(matches!(received.kind, MessageKind::Request));
        
        let receive_again = transport.receive().await;
        assert!(receive_again.is_err());
        assert!(matches!(receive_again.unwrap_err(), TransportError::ReceiveError(_)));
    }

    #[tokio::test]
    async fn test_mock_transport_not_connected() {
        let transport = MockTransport::default();
        
        let msg = Message::new_request("1".into(), "test".into(), json!({}));

        let send_result = transport.send(msg).await;
        assert!(send_result.is_err());
        assert!(matches!(send_result.unwrap_err(), TransportError::ConnectionError(_)));

        let receive_result = transport.receive().await;
        assert!(receive_result.is_err());
        assert!(matches!(receive_result.unwrap_err(), TransportError::ConnectionError(_)));
    }
} 