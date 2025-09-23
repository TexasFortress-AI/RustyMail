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
use std::collections::HashMap;
use uuid::Uuid;
use log::{info, debug, error, warn};
use crate::dashboard::services::metrics::MetricsService;
use crate::dashboard::services::clients::ClientManager;
use crate::dashboard::services::events::{EventBus, DashboardEvent};
use chrono::Utc;
use tokio_stream::wrappers::{ReceiverStream, IntervalStream};
use crate::dashboard::services::DashboardState;
use actix_web::HttpRequest;

// SSE Event data structure
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
}

// SSE client information
#[derive(Debug)]
struct SseClient {
    sender: mpsc::Sender<SseEvent>,
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
        clients.insert(client_id.clone(), SseClient { sender: sender.clone() });
        
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
    
    // Broadcast an event to all connected clients
    pub async fn broadcast(&self, event: SseEvent) {
        let clients = self.clients.read().await;
        
        for (client_id, client) in clients.iter() {
            if let Err(_) = client.sender.send(event.clone()).await {
                debug!("Failed to send event to client {}", client_id);
                // We'll handle client removal on the next heartbeat
            }
        }
    }
    
    // Broadcast a stats updated event
    pub async fn broadcast_stats_updated(&self) {
        let stats = self.metrics_service.get_current_stats().await;
        
        let event = SseEvent {
            event_type: "stats_updated".to_string(),
            data: serde_json::to_string(&stats).unwrap_or_else(|e| {
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
                            let data = json!({
                                "stats": stats,
                                "timestamp": timestamp.to_rfc3339(),
                            });
                            SseEvent {
                                event_type: "metrics_updated".to_string(),
                                data: serde_json::to_string(&data).unwrap_or_default(),
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
            loop {
                interval.tick().await;

                // Get current stats
                let stats = dashboard_state.metrics_service.get_current_stats().await;

                // Serialize to JSON
                match serde_json::to_string(&stats) {
                    Ok(json) => {
                        // Create event and broadcast
                        let event = SseEvent {
                            event_type: "stats_update".to_string(),
                            data: json,
                        };
                        sse_manager.broadcast(event).await;
                    }
                    Err(e) => {
                        warn!("Failed to serialize stats for SSE broadcast: {}", e);
                    }
                }
            }
        });

        info!("Started stats broadcast for SSE clients");
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
    
    // Register client with the SSE manager (NO welcome message here now)
    sse_manager.register_client(client_id.clone(), tx.clone()).await;

    // --- Send Welcome Message Immediately --- 
    let welcome_event = SseEvent {
        event_type: "welcome".to_string(),
        data: format!(r#"{{"clientId":"{}","message":"Connected to RustyMail SSE"}}"#, client_id_clone),
    };
    if let Err(_) = tx.send(welcome_event).await {
        warn!("Failed to send initial welcome message to client {} in handler", client_id_clone);
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

    // Register client with the client manager and use the same client_id
    let client_manager = Arc::clone(&state.client_manager);
    // Extract relevant info from request headers
    let user_agent = req.headers().get(actix_web::http::header::USER_AGENT).and_then(|h| h.to_str().ok()).map(String::from);
    let ip_address = req.peer_addr().map(|addr| addr.ip().to_string());

    // Register client with the client manager (use async but don't spawn task to keep client_id)
    // We need to overwrite the auto-generated client_id with the one from ClientManager
    let managed_client_id = client_manager.register_client(
        crate::dashboard::api::models::ClientType::Sse,
        ip_address,
        user_agent
    ).await;

    // Update SSE manager to use the managed client ID
    sse_manager.remove_client(&client_id).await; // Remove the auto-generated one
    sse_manager.register_client(managed_client_id.clone(), tx.clone()).await;
    
    // Return SSE streaming response
    Sse::from_stream(stream)
}
