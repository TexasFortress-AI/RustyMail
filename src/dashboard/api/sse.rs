// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::convert::Infallible;
use std::time::Duration;
use actix_web::web;
use actix_web_lab::sse::{self, Sse};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use log::{info, debug, error, warn};
use crate::dashboard::services::metrics::MetricsService;
use crate::dashboard::services::clients::ClientManager;
use crate::dashboard::services::events::{EventBus, DashboardEvent};
use chrono::Utc;
use tokio_stream::wrappers::{ReceiverStream, IntervalStream};
use crate::dashboard::services::DashboardState;
use actix_web::HttpRequest;

// Event type definitions for subscription filtering
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    Welcome,
    StatsUpdate,
    ClientConnected,
    ClientDisconnected,
    SystemAlert,
    ConfigurationUpdated,
    DashboardEvent,
}

impl EventType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "welcome" => Some(EventType::Welcome),
            "stats_update" => Some(EventType::StatsUpdate),
            "client_connected" => Some(EventType::ClientConnected),
            "client_disconnected" => Some(EventType::ClientDisconnected),
            "system_alert" => Some(EventType::SystemAlert),
            "configuration_updated" => Some(EventType::ConfigurationUpdated),
            "dashboard_event" => Some(EventType::DashboardEvent),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            EventType::Welcome => "welcome",
            EventType::StatsUpdate => "stats_update",
            EventType::ClientConnected => "client_connected",
            EventType::ClientDisconnected => "client_disconnected",
            EventType::SystemAlert => "system_alert",
            EventType::ConfigurationUpdated => "configuration_updated",
            EventType::DashboardEvent => "dashboard_event",
        }
    }
}

// Default subscription: all events except welcome (which is sent once anyway)
impl Default for EventType {
    fn default() -> Self {
        EventType::StatsUpdate
    }
}

// SSE Event data structure with ID for replay support
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub id: String,  // Event ID for reconnection support
    pub event_type: String,
    pub data: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SseEvent {
    pub fn new(event_type: String, data: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            data,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn new_with_id(id: String, event_type: String, data: String) -> Self {
        Self {
            id,
            event_type,
            data,
            timestamp: chrono::Utc::now(),
        }
    }
}

// SSE client information with subscription preferences
#[derive(Debug)]
struct SseClient {
    sender: mpsc::Sender<SseEvent>,
    subscriptions: HashSet<EventType>,
}

impl SseClient {
    pub fn new(sender: mpsc::Sender<SseEvent>) -> Self {
        // Default subscription: all event types except welcome (sent once on connection)
        let mut subscriptions = HashSet::new();
        subscriptions.insert(EventType::StatsUpdate);
        subscriptions.insert(EventType::ClientConnected);
        subscriptions.insert(EventType::ClientDisconnected);
        subscriptions.insert(EventType::SystemAlert);
        subscriptions.insert(EventType::ConfigurationUpdated);
        subscriptions.insert(EventType::DashboardEvent);

        Self {
            sender,
            subscriptions,
        }
    }

    pub fn new_with_subscriptions(sender: mpsc::Sender<SseEvent>, subscriptions: HashSet<EventType>) -> Self {
        Self {
            sender,
            subscriptions,
        }
    }

    pub fn is_subscribed_to(&self, event_type: &EventType) -> bool {
        self.subscriptions.contains(event_type)
    }

    pub fn subscribe_to(&mut self, event_type: EventType) {
        self.subscriptions.insert(event_type);
    }

    pub fn unsubscribe_from(&mut self, event_type: &EventType) {
        self.subscriptions.remove(event_type);
    }

    pub fn get_subscriptions(&self) -> &HashSet<EventType> {
        &self.subscriptions
    }
}

// Event store for replay functionality
#[derive(Debug, Clone)]
struct StoredEvent {
    event: SseEvent,
    // Store which clients have received this event (for targeted replay)
    delivered_to: HashSet<String>,
}

// Constants for event replay
const MAX_STORED_EVENTS: usize = 100;  // Keep last 100 events
const EVENT_REPLAY_WINDOW: i64 = 300;  // 5 minutes in seconds

// SSE Manager that keeps track of connected clients
pub struct SseManager {
    clients: Arc<RwLock<HashMap<String, SseClient>>>,
    metrics_service: Arc<MetricsService>,
    client_manager: Arc<ClientManager>,
    event_bus: Option<Arc<EventBus>>,
    // Event store for reconnection replay
    event_store: Arc<RwLock<VecDeque<StoredEvent>>>,
}

impl SseManager {
    pub fn new(metrics_service: Arc<MetricsService>, client_manager: Arc<ClientManager>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            metrics_service,
            client_manager,
            event_bus: None,
            event_store: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    // Set the event bus for event integration
    pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
        self.event_bus = Some(event_bus);
    }
    
    // Get the current count of active SSE clients
    pub async fn get_active_client_count(&self) -> usize {
        self.clients.read().await.len()
    }
    
    // Register a new SSE client
    pub async fn register_client(&self, client_id: String, sender: mpsc::Sender<SseEvent>) {
        let mut clients = self.clients.write().await;
        clients.insert(client_id.clone(), SseClient::new(sender));
        
        // Update metrics
        // self.metrics_service.increment_connections().await; // Removed: Metrics now use IMAP count
        
        info!("New SSE client registered: {}", client_id);
        
        // Don't send welcome message from here anymore
        // // Send welcome message immediately
        // self.send_welcome_message(client_id, sender).await;
    }
    
    // Remove a client when they disconnect
    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(client_id);
        // Update metrics
        // self.metrics_service.decrement_connections().await; // Removed: Metrics now use IMAP count
        info!("SSE client disconnected: {}", client_id);
        
        // Broadcast client disconnected event
        self.broadcast_client_disconnected(client_id).await;
    }

    // Update client subscription preferences
    pub async fn update_client_subscriptions(&self, client_id: &str, subscriptions: HashSet<EventType>) -> bool {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.subscriptions = subscriptions;
            info!("Updated subscriptions for client {}: {:?}", client_id, client.subscriptions);
            true
        } else {
            warn!("Tried to update subscriptions for non-existent client: {}", client_id);
            false
        }
    }

    // Add subscription for a client
    pub async fn subscribe_client_to_event(&self, client_id: &str, event_type: EventType) -> bool {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.subscribe_to(event_type.clone());
            info!("Client {} subscribed to {:?}", client_id, event_type);
            true
        } else {
            warn!("Tried to subscribe non-existent client: {}", client_id);
            false
        }
    }

    // Remove subscription for a client
    pub async fn unsubscribe_client_from_event(&self, client_id: &str, event_type: &EventType) -> bool {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.unsubscribe_from(event_type);
            info!("Client {} unsubscribed from {:?}", client_id, event_type);
            true
        } else {
            warn!("Tried to unsubscribe non-existent client: {}", client_id);
            false
        }
    }

    // Get client's current subscriptions
    pub async fn get_client_subscriptions(&self, client_id: &str) -> Option<HashSet<EventType>> {
        let clients = self.clients.read().await;
        clients.get(client_id).map(|client| client.subscriptions.clone())
    }

    // Store an event for potential replay
    async fn store_event(&self, event: &SseEvent, delivered_to: HashSet<String>) {
        let mut store = self.event_store.write().await;

        // Remove old events if we exceed the limit
        while store.len() >= MAX_STORED_EVENTS {
            store.pop_front();
        }

        // Also remove events older than the replay window
        let cutoff_time = chrono::Utc::now() - chrono::Duration::seconds(EVENT_REPLAY_WINDOW);
        while let Some(front) = store.front() {
            if front.event.timestamp < cutoff_time {
                store.pop_front();
            } else {
                break;
            }
        }

        // Add the new event
        store.push_back(StoredEvent {
            event: event.clone(),
            delivered_to,
        });

        debug!("Stored event {} for replay, store size: {}", event.id, store.len());
    }

    // Get events for replay based on Last-Event-ID
    pub async fn get_replay_events(&self, last_event_id: Option<&str>, client_id: &str) -> Vec<SseEvent> {
        let store = self.event_store.read().await;
        let clients = self.clients.read().await;

        // Get client's subscriptions for filtering
        let subscriptions = clients.get(client_id)
            .map(|c| c.subscriptions.clone())
            .unwrap_or_else(|| {
                // Default subscriptions if client not found
                let mut subs = HashSet::new();
                subs.insert(EventType::StatsUpdate);
                subs.insert(EventType::ClientConnected);
                subs.insert(EventType::ClientDisconnected);
                subs.insert(EventType::SystemAlert);
                subs.insert(EventType::ConfigurationUpdated);
                subs.insert(EventType::DashboardEvent);
                subs
            });

        let mut replay_events = Vec::new();
        let mut found_last_event = last_event_id.is_none(); // If no last_event_id, replay all recent events

        for stored_event in store.iter() {
            // Skip until we find the last event ID
            if !found_last_event {
                if Some(stored_event.event.id.as_str()) == last_event_id {
                    found_last_event = true;
                }
                continue; // Skip this event and all before it
            }

            // Check if this event type should be sent to this client
            if let Some(event_type) = EventType::from_string(&stored_event.event.event_type) {
                if subscriptions.contains(&event_type) {
                    // Don't resend events the client already received
                    if !stored_event.delivered_to.contains(client_id) {
                        replay_events.push(stored_event.event.clone());
                    }
                }
            }
        }

        info!("Replaying {} events for reconnected client {}", replay_events.len(), client_id);
        replay_events
    }
    
    // Broadcast an event to all connected clients with filtering
    pub async fn broadcast(&self, event: SseEvent) {
        let clients = self.clients.read().await;

        // Parse event type for filtering
        let event_type = EventType::from_string(&event.event_type);

        // Track which clients receive this event
        let mut delivered_to = HashSet::new();

        for (client_id, client) in clients.iter() {
            // Check if client is subscribed to this event type
            let should_send = match &event_type {
                Some(et) => client.is_subscribed_to(et),
                None => {
                    // Unknown event type - send to all clients (backward compatibility)
                    warn!("Unknown event type '{}', sending to all clients", event.event_type);
                    true
                }
            };

            if should_send {
                if let Err(_) = client.sender.send(event.clone()).await {
                    debug!("Failed to send event to client {}", client_id);
                    // We'll handle client removal on the next heartbeat
                } else {
                    delivered_to.insert(client_id.clone());
                }
            } else {
                debug!("Filtered out event '{}' for client {} (not subscribed)", event.event_type, client_id);
            }
        }

        // Store event for potential replay (but not welcome events)
        if event.event_type != "welcome" {
            self.store_event(&event, delivered_to).await;
        }
    }

    // Broadcast an event to a specific client (bypasses subscription filtering)
    pub async fn send_to_client(&self, client_id: &str, event: SseEvent) -> bool {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(client_id) {
            match client.sender.send(event.clone()).await {
                Ok(_) => {
                    debug!("Sent event '{}' to client {}", event.event_type, client_id);
                    true
                }
                Err(_) => {
                    debug!("Failed to send event to client {}", client_id);
                    false
                }
            }
        } else {
            warn!("Tried to send event to non-existent client: {}", client_id);
            false
        }
    }
    
    // Broadcast a stats updated event
    pub async fn broadcast_stats_updated(&self) {
        let stats = self.metrics_service.get_current_stats().await;

        // Transform stats for SSE compatibility (add active_connections field expected by tests)
        let sse_stats = json!({
            "active_dashboard_sse_clients": stats.active_dashboard_sse_clients,
            "active_connections": stats.active_dashboard_sse_clients, // For test compatibility
            "requests_per_minute": stats.requests_per_minute,
            "average_response_time_ms": stats.average_response_time_ms,
            "system_health": stats.system_health,
            "last_updated": stats.last_updated,
        });

        let event = SseEvent::new(
            "stats_update".to_string(),
            serde_json::to_string(&sse_stats).unwrap_or_else(|e| {
                error!("Failed to serialize stats: {}", e);
                "{}".to_string()
            })
        );

        self.broadcast(event).await;
    }
    
    // Broadcast a client connected event
    pub async fn broadcast_client_connected(&self, client_id: &str) {
        let data = json!({
            "client": {
                "id": client_id,
                "type": "SSE",
                "connectedAt": Utc::now().to_rfc3339(),
                "status": "Active",
            }
        });
        
        let event = SseEvent::new(
            "client_connected".to_string(),
            serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize client connected data: {}", e);
                "{}".to_string()
            })
        );
        
        self.broadcast(event).await;
    }
    
    // Broadcast a client disconnected event
    pub async fn broadcast_client_disconnected(&self, client_id: &str) {
        let data = json!({
            "client": {
                "id": client_id,
                "disconnectedAt": Utc::now().to_rfc3339(),
            }
        });
        
        let event = SseEvent::new(
            "client_disconnected".to_string(),
            serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize client disconnected data: {}", e);
                "{}".to_string()
            })
        );
        
        self.broadcast(event).await;
    }
    
    // Broadcast a system alert
    pub async fn broadcast_system_alert(&self, alert_type: &str, message: &str) {
        let data = json!({
            "type": alert_type,
            "message": message,
            "timestamp": Utc::now().to_rfc3339(),
        });
        
        let event = SseEvent::new(
            "system_alert".to_string(),
            serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize system alert data: {}", e);
                "{}".to_string()
            })
        );
        
        self.broadcast(event).await;
    }
    
    // Start listening to the event bus and forward events to SSE clients
    pub async fn start_event_bus_listener(&self) {
        if let Some(event_bus) = &self.event_bus {
            let sse_manager = Arc::new(self.clone());
            let mut subscription = event_bus.subscribe().await;

            tokio::spawn(async move {
                info!("Started event bus listener for SSE broadcasting");

                while let Some(event) = subscription.recv().await {
                    // Convert DashboardEvent to SseEvent
                    let sse_event = match event {
                        DashboardEvent::MetricsUpdated { stats, timestamp } => {
                            // Transform stats for SSE compatibility (add active_connections field expected by tests)
                            let sse_stats = json!({
                                "active_dashboard_sse_clients": stats.active_dashboard_sse_clients,
                                "active_connections": stats.active_dashboard_sse_clients, // For test compatibility
                                "requests_per_minute": stats.requests_per_minute,
                                "average_response_time_ms": stats.average_response_time_ms,
                                "system_health": stats.system_health,
                                "last_updated": stats.last_updated,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent::new(
                                "stats_update".to_string(),
                                serde_json::to_string(&sse_stats).unwrap_or_default()
                            )
                        },
                        DashboardEvent::ClientConnected { client_id, client_type, ip_address, user_agent, timestamp } => {
                            let data = json!({
                                "clientId": client_id,
                                "clientType": client_type,
                                "ipAddress": ip_address,
                                "userAgent": user_agent,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent::new(
                                "client_connected".to_string(),
                                serde_json::to_string(&data).unwrap_or_default()
                            )
                        },
                        DashboardEvent::ClientDisconnected { client_id, reason, timestamp } => {
                            let data = json!({
                                "clientId": client_id,
                                "reason": reason,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent::new(
                                "client_disconnected".to_string(),
                                serde_json::to_string(&data).unwrap_or_default()
                            )
                        },
                        DashboardEvent::ConfigurationUpdated { section, changes, timestamp } => {
                            let data = json!({
                                "section": section,
                                "changes": changes,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent::new(
                                "configuration_updated".to_string(),
                                serde_json::to_string(&data).unwrap_or_default()
                            )
                        },
                        DashboardEvent::SystemAlert { level, message, details, timestamp } => {
                            let data = json!({
                                "level": level,
                                "message": message,
                                "details": details,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent::new(
                                "system_alert".to_string(),
                                serde_json::to_string(&data).unwrap_or_default()
                            )
                        },
                        _ => {
                            // For other events, use a generic format
                            SseEvent::new(
                                "dashboard_event".to_string(),
                                serde_json::to_string(&event).unwrap_or_default()
                            )
                        }
                    };

                    // Broadcast to all SSE clients
                    sse_manager.broadcast(sse_event).await;
                }

                warn!("Event bus listener stopped - subscription ended");
            });
        } else {
            warn!("Cannot start event bus listener - no event bus configured");
        }
    }

    // Start background task to broadcast stats periodically
    pub async fn start_stats_broadcast(&self, dashboard_state: web::Data<DashboardState>) {
        let sse_manager = Arc::new(self.clone());

        // Start background task to broadcast stats every 5 seconds
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            // Send initial stats immediately (without waiting for first tick)
            info!("Broadcasting initial stats to SSE clients");
            Self::broadcast_current_stats(&sse_manager, &dashboard_state).await;

            loop {
                interval.tick().await;
                Self::broadcast_current_stats(&sse_manager, &dashboard_state).await;
            }
        });

        info!("Started stats broadcast for SSE clients");
    }

    // Helper method to broadcast current stats
    async fn broadcast_current_stats(sse_manager: &Arc<SseManager>, dashboard_state: &web::Data<DashboardState>) {
        // Get current stats
        let stats = dashboard_state.metrics_service.get_current_stats().await;

        // Transform stats for SSE compatibility (add active_connections field expected by tests)
        let sse_stats = json!({
            "active_dashboard_sse_clients": stats.active_dashboard_sse_clients,
            "active_connections": stats.active_dashboard_sse_clients, // For test compatibility
            "requests_per_minute": stats.requests_per_minute,
            "average_response_time_ms": stats.average_response_time_ms,
            "system_health": stats.system_health,
            "last_updated": stats.last_updated,
        });

        // Serialize to JSON
        match serde_json::to_string(&sse_stats) {
            Ok(json) => {
                // Create event and broadcast
                let event = SseEvent::new(
                    "stats_update".to_string(),
                    json
                );
                debug!("Broadcasting stats_update event to {} clients", sse_manager.get_active_client_count().await);
                sse_manager.broadcast(event).await;
            }
            Err(e) => {
                warn!("Failed to serialize stats for SSE broadcast: {}", e);
            }
        }
    }
}

// Make SseManager cloneable
impl Clone for SseManager {
    fn clone(&self) -> Self {
        Self {
            clients: Arc::clone(&self.clients),
            metrics_service: Arc::clone(&self.metrics_service),
            client_manager: Arc::clone(&self.client_manager),
            event_bus: self.event_bus.as_ref().map(Arc::clone),
            event_store: Arc::clone(&self.event_store),
        }
    }
}

// SSE event handler endpoint
pub async fn sse_handler(
    state: web::Data<DashboardState>,
    sse_manager: web::Data<Arc<SseManager>>,
    req: HttpRequest,
) -> Sse<impl Stream<Item = Result<sse::Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(100);
    let client_id = Uuid::new_v4().to_string();
    let client_id_clone = client_id.clone(); // Clone for the welcome message
    
    // Register client with the client manager first to get the managed client ID
    let client_manager = Arc::clone(&state.client_manager);
    // Extract relevant info from request headers
    let user_agent = req.headers().get(actix_web::http::header::USER_AGENT).and_then(|h| h.to_str().ok()).map(String::from);
    let ip_address = req.peer_addr().map(|addr| addr.ip().to_string());

    let managed_client_id = client_manager.register_client(
        crate::dashboard::api::models::ClientType::Sse,
        ip_address,
        user_agent
    ).await;

    // Check for Last-Event-ID header (browser reconnection)
    let last_event_id = req.headers()
        .get("Last-Event-ID")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    if let Some(ref last_id) = last_event_id {
        info!("Client {} reconnecting with Last-Event-ID: {}", managed_client_id, last_id);
    }

    // Register client with SSE manager using the managed client ID
    sse_manager.register_client(managed_client_id.clone(), tx.clone()).await;

    // --- Send Welcome Message Immediately ---
    let welcome_event = SseEvent::new(
        "welcome".to_string(),
        format!(r#"{{"clientId":"{}","message":"Connected to RustyMail SSE","reconnect":{}}}"#,
                managed_client_id,
                last_event_id.is_some())
    );
    if let Err(_) = tx.send(welcome_event).await {
        warn!("Failed to send initial welcome message to client {} in handler", managed_client_id);
        // If we can't send the first message, probably futile to continue
        // Consider returning an error response or empty stream here
    }
    // --- End Welcome Message ---

    // Replay missed events if reconnecting
    if last_event_id.is_some() {
        let replay_events = sse_manager.get_replay_events(
            last_event_id.as_deref(),
            &managed_client_id
        ).await;

        for replay_event in replay_events {
            if let Err(_) = tx.send(replay_event).await {
                warn!("Failed to send replay event to client {}", managed_client_id);
                break;
            }
        }
    }

    // Convert the receiver to a stream
    let event_stream = ReceiverStream::new(rx)
        .map(move |event: SseEvent| {
            // Create event using Data::new and event type, include ID for reconnection support
            let sse_event = sse::Event::Data(
                sse::Data::new(&*event.data)
                    .event(&*event.event_type)
                    .id(&*event.id)
            );
            Ok::<_, Infallible>(sse_event)
        });

    // Create a heartbeat stream
    let heartbeat_interval = IntervalStream::new(interval(Duration::from_secs(15)))
        .map(|_| {
            // Create event comment
            let event = sse::Event::Comment("heartbeat".into());
            Ok::<_, Infallible>(event)
        });

    // Merge the event stream and heartbeat stream
    let stream = futures::stream::select(event_stream, heartbeat_interval);

    // Create a cleanup-aware stream that handles disconnection
    let managed_client_id_for_cleanup = managed_client_id.clone();
    let sse_manager_for_cleanup = Arc::clone(&sse_manager);
    let client_manager_for_cleanup = Arc::clone(&client_manager);

    let cleanup_stream = stream.chain(futures::stream::once(async move {
        // This runs when the stream ends (client disconnects)
        info!("SSE client {} disconnected - performing cleanup", managed_client_id_for_cleanup);

        // Remove from SSE manager
        sse_manager_for_cleanup.remove_client(&managed_client_id_for_cleanup).await;

        // Remove from client manager
        client_manager_for_cleanup.remove_client(&managed_client_id_for_cleanup).await;

        info!("Cleanup completed for disconnected SSE client {}", managed_client_id_for_cleanup);

        // Return a final event (this will never be sent since stream is ending)
        Ok::<_, Infallible>(sse::Event::Comment("cleanup".into()))
    }));

    // Return SSE streaming response with cleanup handling
    Sse::from_stream(cleanup_stream)
}
