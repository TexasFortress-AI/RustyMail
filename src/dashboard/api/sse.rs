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
use std::collections::{HashMap, HashSet};
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

// SSE Event data structure
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
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

// SSE Manager that keeps track of connected clients
pub struct SseManager {
    clients: Arc<RwLock<HashMap<String, SseClient>>>,
    metrics_service: Arc<MetricsService>,
    client_manager: Arc<ClientManager>,
    event_bus: Option<Arc<EventBus>>,
}

impl SseManager {
    pub fn new(metrics_service: Arc<MetricsService>, client_manager: Arc<ClientManager>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            metrics_service,
            client_manager,
            event_bus: None,
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
    
    // Broadcast an event to all connected clients with filtering
    pub async fn broadcast(&self, event: SseEvent) {
        let clients = self.clients.read().await;

        // Parse event type for filtering
        let event_type = EventType::from_string(&event.event_type);

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
                }
            } else {
                debug!("Filtered out event '{}' for client {} (not subscribed)", event.event_type, client_id);
            }
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

        let event = SseEvent {
            event_type: "stats_update".to_string(),
            data: serde_json::to_string(&sse_stats).unwrap_or_else(|e| {
                error!("Failed to serialize stats: {}", e);
                "{}".to_string()
            }),
        };

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
        
        let event = SseEvent {
            event_type: "client_connected".to_string(),
            data: serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize client connected data: {}", e);
                "{}".to_string()
            }),
        };
        
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
        
        let event = SseEvent {
            event_type: "client_disconnected".to_string(),
            data: serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize client disconnected data: {}", e);
                "{}".to_string()
            }),
        };
        
        self.broadcast(event).await;
    }
    
    // Broadcast a system alert
    pub async fn broadcast_system_alert(&self, alert_type: &str, message: &str) {
        let data = json!({
            "type": alert_type,
            "message": message,
            "timestamp": Utc::now().to_rfc3339(),
        });
        
        let event = SseEvent {
            event_type: "system_alert".to_string(),
            data: serde_json::to_string(&data).unwrap_or_else(|e| {
                error!("Failed to serialize system alert data: {}", e);
                "{}".to_string()
            }),
        };
        
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
                            SseEvent {
                                event_type: "stats_update".to_string(),
                                data: serde_json::to_string(&sse_stats).unwrap_or_default(),
                            }
                        },
                        DashboardEvent::ClientConnected { client_id, client_type, ip_address, user_agent, timestamp } => {
                            let data = json!({
                                "clientId": client_id,
                                "clientType": client_type,
                                "ipAddress": ip_address,
                                "userAgent": user_agent,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent {
                                event_type: "client_connected".to_string(),
                                data: serde_json::to_string(&data).unwrap_or_default(),
                            }
                        },
                        DashboardEvent::ClientDisconnected { client_id, reason, timestamp } => {
                            let data = json!({
                                "clientId": client_id,
                                "reason": reason,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent {
                                event_type: "client_disconnected".to_string(),
                                data: serde_json::to_string(&data).unwrap_or_default(),
                            }
                        },
                        DashboardEvent::ConfigurationUpdated { section, changes, timestamp } => {
                            let data = json!({
                                "section": section,
                                "changes": changes,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent {
                                event_type: "configuration_updated".to_string(),
                                data: serde_json::to_string(&data).unwrap_or_default(),
                            }
                        },
                        DashboardEvent::SystemAlert { level, message, details, timestamp } => {
                            let data = json!({
                                "level": level,
                                "message": message,
                                "details": details,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent {
                                event_type: "system_alert".to_string(),
                                data: serde_json::to_string(&data).unwrap_or_default(),
                            }
                        },
                        _ => {
                            // For other events, use a generic format
                            SseEvent {
                                event_type: "dashboard_event".to_string(),
                                data: serde_json::to_string(&event).unwrap_or_default(),
                            }
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
                let event = SseEvent {
                    event_type: "stats_update".to_string(),
                    data: json,
                };
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

    // Register client with SSE manager using the managed client ID
    sse_manager.register_client(managed_client_id.clone(), tx.clone()).await;

    // --- Send Welcome Message Immediately ---
    let welcome_event = SseEvent {
        event_type: "welcome".to_string(),
        data: format!(r#"{{"clientId":"{}","message":"Connected to RustyMail SSE"}}"#, managed_client_id),
    };
    if let Err(_) = tx.send(welcome_event).await {
        warn!("Failed to send initial welcome message to client {} in handler", managed_client_id);
        // If we can't send the first message, probably futile to continue
        // Consider returning an error response or empty stream here
    }
    // --- End Welcome Message ---

    // Convert the receiver to a stream
    let event_stream = ReceiverStream::new(rx)
        .map(move |event: SseEvent| {
            // Create event using Data::new and event type
            let sse_event = sse::Event::Data(
                sse::Data::new(&*event.data)
                    .event(&*event.event_type)
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
