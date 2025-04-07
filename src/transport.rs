use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Option<String>,
    pub kind: MessageKind,
    pub payload: serde_json::Value,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum MessageKind {
    Request,
    Response,
    Notification,
    Error,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Failed to send message: {0}")]
    SendError(String),
    #[error("Failed to receive message: {0}")]
    ReceiveError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a message through the transport
    async fn send(&self, message: Message) -> Result<(), TransportError>;
    
    /// Receive a message from the transport
    async fn receive(&self) -> Result<Message, TransportError>;
    
    /// Close the transport connection
    async fn close(&self) -> Result<(), TransportError>;
    
    /// Check if the transport is connected
    async fn is_connected(&self) -> bool;
}

// Helper functions for message creation
impl Message {
    pub fn new_request(id: String, payload: serde_json::Value) -> Self {
        Self {
            id: Some(id),
            kind: MessageKind::Request,
            payload,
        }
    }

    pub fn new_response(id: String, payload: serde_json::Value) -> Self {
        Self {
            id: Some(id),
            kind: MessageKind::Response,
            payload,
        }
    }

    pub fn new_notification(payload: serde_json::Value) -> Self {
        Self {
            id: None,
            kind: MessageKind::Notification,
            payload,
        }
    }

    pub fn new_error(id: Option<String>, error: impl StdError) -> Self {
        Self {
            id,
            kind: MessageKind::Error,
            payload: serde_json::json!({
                "error": error.to_string()
            }),
        }
    }
} 