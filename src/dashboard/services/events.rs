// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Event Broadcasting System for Dashboard Services
//
// This module provides a centralized event bus for coordinating events
// between different dashboard services and broadcasting them to SSE clients.

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use std::collections::HashMap;
use uuid::Uuid;

// Event types that can be broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DashboardEvent {
    // Metrics events
    MetricsUpdated {
        stats: crate::dashboard::api::models::DashboardStats,
        timestamp: DateTime<Utc>,
    },

    // Client events
    ClientConnected {
        client_id: String,
        client_type: crate::dashboard::api::models::ClientType,
        ip_address: Option<String>,
        user_agent: Option<String>,
        timestamp: DateTime<Utc>,
    },
    ClientDisconnected {
        client_id: String,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    ClientStatusChanged {
        client_id: String,
        old_status: crate::dashboard::api::models::ClientStatus,
        new_status: crate::dashboard::api::models::ClientStatus,
        timestamp: DateTime<Utc>,
    },

    // Configuration events
    ConfigurationUpdated {
        section: ConfigSection,
        changes: HashMap<String, serde_json::Value>,
        timestamp: DateTime<Utc>,
    },
    ConfigurationError {
        section: ConfigSection,
        error: String,
        timestamp: DateTime<Utc>,
    },

    // IMAP events
    ImapSessionCreated {
        session_id: String,
        account: String,
        timestamp: DateTime<Utc>,
    },
    ImapSessionClosed {
        session_id: String,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    ImapOperationCompleted {
        session_id: String,
        operation: String,
        success: bool,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // System events
    SystemAlert {
        level: AlertLevel,
        message: String,
        details: Option<serde_json::Value>,
        timestamp: DateTime<Utc>,
    },
    SystemHealthChanged {
        healthy: bool,
        issues: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    // AI Assistant events
    AiQueryReceived {
        query_id: String,
        query: String,
        timestamp: DateTime<Utc>,
    },
    AiResponseGenerated {
        query_id: String,
        response: String,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    AiError {
        query_id: Option<String>,
        error: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSection {
    Imap,
    Rest,
    Dashboard,
    Sse,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

// Event subscription handle
pub struct Subscription {
    id: String,
    receiver: mpsc::UnboundedReceiver<DashboardEvent>,
}

impl Subscription {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn recv(&mut self) -> Option<DashboardEvent> {
        self.receiver.recv().await
    }
}

// Event filter for selective subscriptions
#[derive(Debug, Clone)]
pub enum EventFilter {
    All,
    EventType(Vec<String>),
    ClientId(String),
    SessionId(String),
}

// Event Bus for managing event distribution
pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<DashboardEvent>>>>,
    event_history: Arc<RwLock<Vec<DashboardEvent>>>,
    max_history_size: usize,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 100, // Keep last 100 events
        }
    }

    // Subscribe to all events
    pub async fn subscribe(&self) -> Subscription {
        let (tx, rx) = mpsc::unbounded_channel();
        let id = Uuid::new_v4().to_string();

        let mut subscribers = self.subscribers.write().await;
        subscribers.insert(id.clone(), tx);

        info!("New event bus subscription: {}", id);

        Subscription { id, receiver: rx }
    }

    // Unsubscribe from events
    pub async fn unsubscribe(&self, subscription_id: &str) {
        let mut subscribers = self.subscribers.write().await;
        if subscribers.remove(subscription_id).is_some() {
            info!("Event bus subscription removed: {}", subscription_id);
        }
    }

    // Publish an event to all subscribers
    pub async fn publish(&self, event: DashboardEvent) {
        debug!("Publishing event: {:?}", event);

        // Add to history
        {
            let mut history = self.event_history.write().await;
            history.push(event.clone());

            // Trim history if needed
            let history_len = history.len();
            if history_len > self.max_history_size {
                history.drain(0..(history_len - self.max_history_size));
            }
        }

        // Send to all subscribers
        let subscribers = self.subscribers.read().await;
        let mut failed_subscribers = Vec::new();

        for (id, sender) in subscribers.iter() {
            if let Err(e) = sender.send(event.clone()) {
                warn!("Failed to send event to subscriber {}: {}", id, e);
                failed_subscribers.push(id.clone());
            }
        }

        // Clean up failed subscribers
        if !failed_subscribers.is_empty() {
            drop(subscribers);
            let mut subscribers = self.subscribers.write().await;
            for id in failed_subscribers {
                subscribers.remove(&id);
                warn!("Removed failed subscriber: {}", id);
            }
        }
    }

    // Get recent event history
    pub async fn get_history(&self, count: usize) -> Vec<DashboardEvent> {
        let history = self.event_history.read().await;
        let start = if history.len() > count {
            history.len() - count
        } else {
            0
        };
        history[start..].to_vec()
    }

    // Clear event history
    pub async fn clear_history(&self) {
        let mut history = self.event_history.write().await;
        history.clear();
        info!("Event history cleared");
    }

    // Get subscriber count
    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

// Make EventBus cloneable
impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            event_history: Arc::clone(&self.event_history),
            max_history_size: self.max_history_size,
        }
    }
}

// Event builder helpers for common events
impl EventBus {
    pub async fn publish_metrics_updated(&self, stats: crate::dashboard::api::models::DashboardStats) {
        self.publish(DashboardEvent::MetricsUpdated {
            stats,
            timestamp: Utc::now(),
        }).await;
    }

    pub async fn publish_client_connected(
        &self,
        client_id: String,
        client_type: crate::dashboard::api::models::ClientType,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) {
        self.publish(DashboardEvent::ClientConnected {
            client_id,
            client_type,
            ip_address,
            user_agent,
            timestamp: Utc::now(),
        }).await;
    }

    pub async fn publish_client_disconnected(&self, client_id: String, reason: Option<String>) {
        self.publish(DashboardEvent::ClientDisconnected {
            client_id,
            reason,
            timestamp: Utc::now(),
        }).await;
    }

    pub async fn publish_system_alert(&self, level: AlertLevel, message: String, details: Option<serde_json::Value>) {
        self.publish(DashboardEvent::SystemAlert {
            level,
            message,
            details,
            timestamp: Utc::now(),
        }).await;
    }

    pub async fn publish_configuration_updated(
        &self,
        section: ConfigSection,
        changes: HashMap<String, serde_json::Value>,
    ) {
        self.publish(DashboardEvent::ConfigurationUpdated {
            section,
            changes,
            timestamp: Utc::now(),
        }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_subscribe_publish() {
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

        // Get history
        let history = event_bus.get_history(3).await;
        assert_eq!(history.len(), 3);

        // Verify we got the most recent events
        match &history[2] {
            DashboardEvent::SystemAlert { message, .. } => {
                assert_eq!(message, "Alert 4");
            }
            _ => panic!("Unexpected event type"),
        }
    }
}