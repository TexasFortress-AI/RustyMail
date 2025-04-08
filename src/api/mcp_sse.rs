use actix_web::{web, Responder, HttpResponse, Error, HttpRequest};
use actix_web::rt::time::interval;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use actix_web_lab::sse::{Sse, Data as SseData, Event as SseEvent};
use futures_util::stream::{StreamExt, once, Stream};
use tokio_stream::wrappers::ReceiverStream;
use std::convert::Infallible;
use std::pin::Pin;
use crate::imap::client::ImapClient;
use crate::mcp_port::{McpTool, McpPortError};

#[derive(Deserialize, Serialize, Debug)]
struct SseCommandPayload {
    client_id: Option<usize>,
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
pub struct SseState {
    clients: HashMap<usize, SseClient>,
    visitor_count: usize,
}

impl SseState {
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

// SSE Adapter Structure
pub struct SseAdapter {
    // Removed unused fields
    // shared_state: Arc<TokioMutex<SseState>>,
    // rest_config: RestConfig, 
}

// SSE Adapter Implementation
impl SseAdapter {
    pub fn new(
        // Removed unused parameters
        // shared_state: Arc<TokioMutex<SseState>>,
        // rest_config: RestConfig,
    ) -> Self {
        SseAdapter { 
            // Removed unused fields
            // shared_state,
            // rest_config
        }
    }

    // Handler for establishing SSE connection
    async fn sse_connect_handler(state: web::Data<Arc<TokioMutex<SseState>>>) -> impl Responder {
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
        sse_state: web::Data<Arc<TokioMutex<SseState>>>,
        // Placeholder: Assume Tool Registry is shared via web::Data
        // tool_registry: web::Data<Arc<HashMap<String, Arc<dyn McpTool>>>>,
        // Placeholder: Assume ImapClient is shared via web::Data (if tools need it)
        // imap_client: web::Data<Arc<ImapClient>>,
        payload: web::Json<SseCommandPayload>
    ) -> impl Responder
    {
        info!("Received SSE command: {:?}", payload);
        
        let client_id = match payload.client_id {
            Some(id) => id,
            None => {
                warn!("SSE command received without client_id");
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Missing client_id in command payload"
                }));
            }
        };

        // --- TODO: Implement Tool Execution Logic --- 
        // 1. Get tool registry & imap client from web::Data
        // 2. Lookup tool by payload.command
        // 3. Execute tool with payload.params (handle errors)
        // 4. Serialize result/error to JSON
        // 5. Find client sender in sse_state using client_id
        // 6. Format as SSE event (tool_result/tool_error)
        // 7. Send targeted message
        // --- End TODO ---

        // --- Placeholder: Broadcast command back (to be replaced) ---
        let broadcast_message = match serde_json::to_string(&payload.into_inner()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize command payload for broadcast: {}", e);
                 return HttpResponse::InternalServerError().finish();
            }
        };
        let state_guard = sse_state.lock().await;
        state_guard.broadcast(&broadcast_message).await;
        // --- End Placeholder ---

        HttpResponse::Accepted().json(serde_json::json!({ "status": "Command received" }))
    }

    // Function to configure SSE routes within an Actix App
    pub fn configure_sse_service(cfg: &mut web::ServiceConfig, state: Arc<TokioMutex<SseState>>) {
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

// --- Remove the unused SseManager and sse_handler --- 
/*
// Type alias for the boxed stream
type SseStream = Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>>;

#[derive(Clone)]
pub struct SseManager {
    sessions: Arc<TokioMutex<HashMap<String, mpsc::Sender<SseEvent>>>>,
}

impl SseManager {
    pub fn new() -> Self {
        Self { sessions: Arc::new(TokioMutex::new(HashMap::new())) }
    }
    // Example method signature 
    // pub async fn send_to_session(&self, session_id: &str, message: SseEvent) { ... }
}

/// SSE endpoint handler - now returns concrete Sse<SseStream> type
pub async fn sse_handler(manager: web::Data<SseManager>, req: HttpRequest) -> Sse<SseStream> { // Explicit return type
    let session_id = match req.match_info().get("session_id") {
        Some(id) => id.to_string(),
        None => {
            error!("Missing session_id in SSE request path");
            let error_event = SseEvent::Data(SseData::new("Error: Missing session_id").event("error"));
            // Box the error stream
            let stream: SseStream = Box::pin(once(async { Ok::<_, Infallible>(error_event) }));
            return Sse::from_stream(stream);
        }
    };

    // Create a tokio mpsc channel for SseEvent
    let (tx, rx) = mpsc::channel::<SseEvent>(10);

    // Register client
    {
        let mut sessions = manager.sessions.lock().await; 
        if sessions.contains_key(&session_id) {
            error!("SSE session ID already exists: {}", session_id);
            let error_event = SseEvent::Data(SseData::new("Error: Session ID exists").event("error"));
            // Box the error stream
            let stream: SseStream = Box::pin(once(async { Ok::<_, Infallible>(error_event) }));
            return Sse::from_stream(stream);
        }
        sessions.insert(session_id.clone(), tx.clone());
        info!("SSE client connected: Session ID {}", session_id);
    }

    // Spawn background task for heartbeats and cleanup
    let manager_clone = manager.clone();
    actix_web::rt::spawn(async move {
        let heartbeat_interval = Duration::from_secs(15);
        let mut interval_timer = interval(heartbeat_interval);
        interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
             tokio::select! {
                _ = interval_timer.tick() => {
                    if tx.send(SseEvent::Comment("heartbeat".into())).await.is_err() {
                        warn!("SSE Client disconnected (heartbeat send failed). Session ID: {}", session_id);
                        break; 
                    }
                }
            }
        }
        // Cleanup session map on disconnect
        info!("SSE connection closing for Session ID: {}", session_id);
        let mut sessions = manager_clone.sessions.lock().await;
        sessions.remove(&session_id);
    });

    // Return the SSE stream on success, boxing it
    let receiver_stream = ReceiverStream::new(rx)
        .map(|event| Ok::<_, Infallible>(event)); // Map to Result<SseEvent, Infallible>
    
    let stream: SseStream = Box::pin(receiver_stream); // Box the success stream
}
*/ 