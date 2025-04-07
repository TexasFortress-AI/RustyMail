#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock transport implementation for testing
    struct MockTransport {
        connected: bool,
        messages: Arc<Mutex<Vec<Message>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                connected: true,
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&self, message: Message) -> Result<(), TransportError> {
            if !self.connected {
                return Err(TransportError::ConnectionError("Not connected".to_string()));
            }
            self.messages.lock().await.push(message);
            Ok(())
        }

        async fn receive(&self) -> Result<Message, TransportError> {
            if !self.connected {
                return Err(TransportError::ConnectionError("Not connected".to_string()));
            }
            self.messages
                .lock()
                .await
                .pop()
                .ok_or_else(|| TransportError::ReceiveError("No messages".to_string()))
        }

        async fn close(&self) -> Result<(), TransportError> {
            Ok(())
        }

        async fn is_connected(&self) -> bool {
            self.connected
        }
    }

    #[tokio::test]
    async fn test_message_creation() {
        let request = Message::new_request(
            "123".to_string(),
            json!({ "command": "list_folders" }),
        );
        assert_eq!(request.id, Some("123".to_string()));
        assert!(matches!(request.kind, MessageKind::Request));

        let response = Message::new_response(
            "123".to_string(),
            json!({ "folders": ["INBOX", "Sent"] }),
        );
        assert_eq!(response.id, Some("123".to_string()));
        assert!(matches!(response.kind, MessageKind::Response));

        let notification = Message::new_notification(
            json!({ "event": "new_mail" }),
        );
        assert_eq!(notification.id, None);
        assert!(matches!(notification.kind, MessageKind::Notification));

        let error = Message::new_error(
            Some("123".to_string()),
            std::io::Error::new(std::io::ErrorKind::Other, "test error"),
        );
        assert_eq!(error.id, Some("123".to_string()));
        assert!(matches!(error.kind, MessageKind::Error));
    }

    #[tokio::test]
    async fn test_transport_send_receive() {
        let transport = MockTransport::new();
        
        // Test sending a message
        let msg = Message::new_request(
            "123".to_string(),
            json!({ "command": "test" }),
        );
        transport.send(msg.clone()).await.unwrap();

        // Test receiving the message
        let received = transport.receive().await.unwrap();
        assert_eq!(received.id, msg.id);
        assert!(matches!(received.kind, MessageKind::Request));
    }

    #[tokio::test]
    async fn test_transport_connection_error() {
        let transport = MockTransport {
            connected: false,
            messages: Arc::new(Mutex::new(Vec::new())),
        };

        let msg = Message::new_request(
            "123".to_string(),
            json!({ "command": "test" }),
        );

        // Test sending when disconnected
        let send_result = transport.send(msg).await;
        assert!(matches!(send_result, Err(TransportError::ConnectionError(_))));

        // Test receiving when disconnected
        let receive_result = transport.receive().await;
        assert!(matches!(receive_result, Err(TransportError::ConnectionError(_))));
    }
} 