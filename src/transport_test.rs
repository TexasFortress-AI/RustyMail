use crate::transport::{Message, MessageKind, Transport, TransportError};
use serde_json::json;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
struct MockTransport {
    messages: Arc<Mutex<Vec<Message>>>,
    connected: bool,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            connected: true,
        }
    }
}

#[async_trait::async_trait]
impl Transport for MockTransport {
    async fn send(&self, message: Message) -> Result<(), TransportError> {
        let mut messages = self.messages.lock().await;
        messages.push(message);
        Ok(())
    }

    async fn receive(&self) -> Result<Message, TransportError> {
        let mut messages = self.messages.lock().await;
        if messages.is_empty() {
            return Err(TransportError::ReceiveError("No messages available".to_string()));
        }
        Ok(messages.remove(0))
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
    // Test request message
    let request = Message::new_request("1".to_string(), json!({"method": "test"}));
    assert_eq!(request.id, Some("1".to_string()));
    assert_eq!(request.kind, MessageKind::Request);
    assert_eq!(request.payload, json!({"method": "test"}));

    // Test response message
    let response = Message::new_response("1".to_string(), json!({"result": "success"}));
    assert_eq!(response.id, Some("1".to_string()));
    assert_eq!(response.kind, MessageKind::Response);
    assert_eq!(response.payload, json!({"result": "success"}));

    // Test notification message
    let notification = Message::new_notification(json!({"event": "update"}));
    assert_eq!(notification.id, None);
    assert_eq!(notification.kind, MessageKind::Notification);
    assert_eq!(notification.payload, json!({"event": "update"}));

    // Test error message
    let error = Message::new_error(Some("1".to_string()), "Test error");
    assert_eq!(error.id, Some("1".to_string()));
    assert_eq!(error.kind, MessageKind::Error);
    assert_eq!(error.payload, json!({"error": "Test error"}));
}

#[tokio::test]
async fn test_transport_send_receive() {
    let transport = MockTransport::new();
    
    // Send a message
    let message = Message::new_request("1".to_string(), json!({"method": "test"}));
    transport.send(message.clone()).await.unwrap();

    // Receive the message
    let received = transport.receive().await.unwrap();
    assert_eq!(received.id, message.id);
    assert_eq!(received.kind, message.kind);
    assert_eq!(received.payload, message.payload);
}

#[tokio::test]
async fn test_transport_empty_receive() {
    let transport = MockTransport::new();
    
    // Try to receive from empty transport
    let result = transport.receive().await;
    assert!(matches!(result, Err(TransportError::ReceiveError(_))));
}

#[tokio::test]
async fn test_transport_connection() {
    let transport = MockTransport::new();
    
    // Check connection status
    assert!(transport.is_connected().await);
    
    // Close connection
    transport.close().await.unwrap();
}

#[tokio::test]
async fn test_transport_multiple_messages() {
    let transport = MockTransport::new();
    
    // Send multiple messages
    let message1 = Message::new_request("1".to_string(), json!({"method": "test1"}));
    let message2 = Message::new_request("2".to_string(), json!({"method": "test2"}));
    
    transport.send(message1.clone()).await.unwrap();
    transport.send(message2.clone()).await.unwrap();

    // Receive messages in order
    let received1 = transport.receive().await.unwrap();
    assert_eq!(received1.id, message1.id);
    
    let received2 = transport.receive().await.unwrap();
    assert_eq!(received2.id, message2.id);
} 