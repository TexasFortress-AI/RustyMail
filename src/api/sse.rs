use actix_web::{web, Responder, HttpResponse, Error};
use actix_web::rt::time::interval;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use crate::config::RestConfig;
use futures::stream::StreamExt;
use async_stream::stream;

#[derive(Deserialize, Serialize, Debug)]
struct SseCommandPayload {
    command: String,
    params: serde_json::Value,
}

// Placeholder for SSE specific config
pub struct SseConfig {
    // Potential config like heartbeat interval
}

// Structure to hold SSE client state
struct SseClient {
    sender: mpsc::Sender<String>,
}

// Shared state for SSE adapter
#[derive(Default)]
struct SseState {
    clients: HashMap<usize, SseClient>,
    visitor_count: usize,
}

impl SseState {
    fn new() -> Self {
        SseState { 
            clients: HashMap::new(),
            visitor_count: 0,
        }
    }

    // Add a client
    fn add_client(&mut self, tx: mpsc::Sender<String>) -> usize {
        self.visitor_count += 1;
        let id = self.visitor_count;
        self.clients.insert(id, SseClient { sender: tx });
        id
    }

    // Remove a client
    fn remove_client(&mut self, id: usize) {
        self.clients.remove(&id);
        info!("SSE Client {} disconnected/removed.", id);
    }

    // Broadcast a message to all clients
    async fn broadcast(&self, message: &str) {
        let mut disconnected_ids = Vec::new();
        for (id, client) in self.clients.iter() {
             // Format as SSE message data
            let sse_msg = format!("data: {}\n\n", message);
            if client.sender.send(sse_msg).await.is_err() {
                warn!("Failed to send message to SSE client {}, assuming disconnected.", id);
                disconnected_ids.push(*id);
            }
        }
        // Defer removal to avoid borrowing issues (not strictly needed with lock release but cleaner)
        // for id in disconnected_ids { self.remove_client(id); }
    }
}

pub struct SseAdapter {
    // Might need IMAP client later
    shared_state: Arc<Mutex<SseState>>,
    // Use imported RestConfig
    rest_config: RestConfig, 
}

impl SseAdapter {
    // Update signature to use imported RestConfig
    pub fn new(rest_config: RestConfig) -> Self {
        Self {
            shared_state: Arc::new(Mutex::new(SseState::new())),
            rest_config,
        }
    }

    // Handler for establishing SSE connection
    async fn sse_connect_handler(state: web::Data<Arc<Mutex<SseState>>>) -> impl Responder {
        let (tx, rx) = mpsc::channel::<String>(10);
        
        let client_id = state.lock().await.add_client(tx.clone());
        info!("New SSE Client connected: {}", client_id);

        // Example: Send a welcome message with client ID
        let welcome_msg = format!("event: welcome\ndata: {{ \"clientId\": {} }}\n\n", client_id);
        if tx.send(welcome_msg).await.is_err() {
            state.lock().await.remove_client(client_id);
            return HttpResponse::InternalServerError().finish();
        }

        let stream = async_stream::stream! {
            let mut interval_stream = interval(Duration::from_secs(10));
            let mut message_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

            loop {
                tokio::select! {
                    // Send heartbeat every 10s
                    _ = interval_stream.tick() => {
                        if tx.send("event: heartbeat\ndata: {}\n\n".to_string()).await.is_err() {
                            break; 
                        }
                    }
                    // Yield messages received on the channel
                    maybe_msg = message_stream.next() => {
                        if let Some(msg) = maybe_msg {
                             yield std::result::Result::<_, Error>::Ok(actix_web::web::Bytes::from(msg));
                        } else {
                            break; // Channel closed
                        }
                    }
                    else => {
                        break;
                    }
                }
            }

            state.lock().await.remove_client(client_id);
        };

        HttpResponse::Ok()
            .insert_header(("Content-Type", "text/event-stream"))
            .insert_header(("Cache-Control", "no-cache"))
            .streaming(stream)
    }

     // Handler for receiving commands via POST
    async fn sse_command_handler(
        state: web::Data<Arc<Mutex<SseState>>>, 
        payload: web::Json<SseCommandPayload>
    ) -> impl Responder 
    {
        info!("Received SSE command: {:?}", payload);
        // 1. TODO: Process the command (e.g., call an McpTool/ImapClient method)
        // 2. For now, just broadcast the received command back to all clients
        let broadcast_message = match serde_json::to_string(&payload.into_inner()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize command payload for broadcast: {}", e);
                 return HttpResponse::InternalServerError().finish();
            }
        };

        let state_guard = state.lock().await;
        state_guard.broadcast(&broadcast_message).await;
        drop(state_guard); // Release lock explicitly if needed elsewhere

        HttpResponse::Accepted().json(serde_json::json!({ "status": "Command received" }))
    }

    // Function to configure SSE routes within an Actix App
    pub fn configure_sse_service(cfg: &mut web::ServiceConfig, state: Arc<Mutex<SseState>>) {
         let app_state = web::Data::new(state);
         cfg.app_data(app_state.clone()); 
         cfg.service(
             web::scope("/api/v1/sse") // Group SSE routes
                 .route("/connect", web::get().to(Self::sse_connect_handler)) // Endpoint to connect
                 .route("/command", web::post().to(Self::sse_command_handler)) // Endpoint for commands
         );
    }

    // Standalone server function (if SSE runs on its own port)
    // pub async fn run_sse_server(...) -> std::io::Result<()> { ... }
} 