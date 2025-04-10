use actix_web::{
    web::{self, Data, Path},
    Error as ActixError, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::sse::{self, Sse, Event};
use futures_util::stream::Stream;
use tokio::{
    sync::{
        Mutex as TokioMutex,
        RwLock,
        mpsc,
        broadcast,
    },
    time::{Duration, interval},
};
use std::{
    sync::Arc,
    collections::HashMap,
};
use uuid::Uuid;
use log::{error, info, debug, warn};
use serde::Serialize;
use serde_json::{self, Value};

// Use re-exported MCP types and traits
use crate::mcp::{
    McpHandler, McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError,
    ErrorCode,
};

// Re-exports for use in other modules (if needed by main.rs or elsewhere)
pub use actix_web::{
    App, HttpServer,
    web, // Keep web module import if other items from it are used
};

#[derive(Debug, Serialize)]
struct SseCommandPayload {
    command: String,
    payload: JsonRpcRequest,
}

#[derive(Debug, Clone, Serialize)]
pub struct SseClientInfo {
    id: Uuid,
    sender: mpsc::Sender<sse::Event>,
}

pub struct SseClient {
    sender: mpsc::Sender<sse::Event>,
    info: SseClientInfo,
}

/// State management for the MCP-over-SSE transport.
///
/// This struct holds the central state for managing active SSE client connections.
/// It maintains a map of client IDs to their corresponding `mpsc::Sender` channels,
/// allowing the server to send events (like responses or notifications) to specific clients.
/// It also holds a reference to the shared `McpHandler` responsible for processing
/// incoming MCP requests received via other means (e.g., a paired REST endpoint or 
/// potentially a websocket, though SSE is primarily for server-to-client). 
///
/// NOTE: The current implementation focuses on setting up the SSE stream for server-to-client
/// communication. Handling client-to-server messages over SSE is less common and
/// would typically involve a separate HTTP endpoint (e.g., POST) where the client
/// sends MCP requests, which are then processed by the `McpHandler`.
/// The `McpHandler` reference here suggests potential integration points for such a setup.
pub struct McpSseState {
    /// A thread-safe map storing active SSE clients.
    /// Key: Unique client identifier (`Uuid`).
    /// Value: An `mpsc::Sender` used to push `sse::Event`s to the client's stream.
    clients: Arc<RwLock<HashMap<Uuid, mpsc::Sender<sse::Event>>>>,
    /// A reference to the central MCP request handler.
    /// Used to process MCP requests (likely received via a separate channel like REST).
    mcp_handler: Arc<dyn McpHandler>,
}

impl McpSseState {
    /// Creates a new `McpSseState`.
    ///
    /// # Arguments
    ///
    /// * `mcp_handler` - An `Arc` pointing to the shared `McpHandler` implementation.
    pub fn new(mcp_handler: Arc<dyn McpHandler>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            mcp_handler,
        }
    }

    /// Adds a new client to the state, creating a communication channel for them.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The unique `Uuid` for the new client.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `mpsc::Receiver` end of the channel if successful.
    /// The server uses this receiver to create the SSE stream for the client.
    /// Returns a `JsonRpcError` (InternalError) if channel creation fails.
    async fn add_client(&self, client_id: Uuid) -> Result<mpsc::Receiver<sse::Event>, JsonRpcError> {
        let (tx, rx) = mpsc::channel(100); // Increased buffer size
        let mut clients_guard = self.clients.write().await;
        if clients_guard.contains_key(&client_id) {
            error!("MCP SSE: Client {} already exists!", client_id);
            // Consider returning a specific error or handling differently
            return Err(JsonRpcError::internal_error(format!("Client ID {} already exists", client_id)));
        }
        clients_guard.insert(client_id, tx);
        info!("MCP SSE: Client {} added successfully.", client_id);
        Ok(rx)
    }

    /// Removes a client from the state, typically when they disconnect.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The `Uuid` of the client to remove.
    async fn remove_client(&self, client_id: &Uuid) {
        if self.clients.write().await.remove(client_id).is_some() {
            info!("MCP SSE: Client {} removed.", client_id);
        } else {
            warn!("MCP SSE: Attempted to remove non-existent client {}.", client_id);
        }
    }

    /// Broadcasts an SSE event to all currently connected clients.
    ///
    /// This is used for sending notifications or updates to everyone subscribed.
    ///
    /// # Arguments
    ///
    /// * `event` - The `sse::Event` to broadcast.
    pub async fn broadcast_event(&self, event: sse::Event) {
        let clients = self.clients.read().await;
        if clients.is_empty() {
            debug!("MCP SSE: No clients connected, skipping broadcast.");
            return;
        }
        debug!("MCP SSE: Broadcasting event to {} clients: {:?}", clients.len(), event);
        // Using join_all to send concurrently might be slightly more efficient for many clients
        let sends = clients.iter().map(|(_, tx)| {
            let event_clone = event.clone();
            async move {
                if let Err(e) = tx.send(event_clone).await {
                    // Log error if sending fails (client might have disconnected)
                    error!("MCP SSE: Failed to send event to client: {}", e);
                }
            }
        });
        futures_util::future::join_all(sends).await;
    }

    /// Sends an SSE event to a specific client.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The `Uuid` of the target client.
    /// * `event` - The `sse::Event` to send.
    ///
    /// # Returns
    ///
    /// `true` if the client was found and the message was sent (or queued), `false` otherwise.
    pub async fn send_event_to_client(&self, client_id: &Uuid, event: sse::Event) -> bool {
        let clients = self.clients.read().await;
        if let Some(tx) = clients.get(client_id) {
            debug!("MCP SSE: Sending event to client {}: {:?}", client_id, event);
            if let Err(e) = tx.send(event).await {
                error!("MCP SSE: Failed to send event to client {}: {}", client_id, e);
                // Consider removing the client here if sending fails repeatedly
                false 
            } else {
                true
            }
        } else {
            warn!("MCP SSE: Attempted to send event to non-existent client {}.", client_id);
            false
        }
    }
}

/// Actix-web handler for establishing an SSE connection.
///
/// This endpoint is typically called by a client to initiate the SSE stream.
/// It adds the client to the `McpSseState` and returns the streaming response.
/// The client ID is expected as part of the URL path.
async fn sse_connection_handler(
    sse_state: web::Data<McpSseState>,
    path: web::Path<Uuid>, // Expect client_id in the path
    req: HttpRequest,
) -> Result<HttpResponse, ActixError> {
    let client_id = path.into_inner();
    info!("MCP SSE: Handling new connection request for client_id: {}", client_id);

    match sse_state.add_client(client_id).await {
        Ok(rx) => {
            info!("MCP SSE: Client {} successfully registered, returning SSE stream.", client_id);
            
            // Create the SSE stream from the receiver
            let stream = Sse::from_infallible_receiver(rx)
                .with_keep_alive(Duration::from_secs(15)); // Send comments as keep-alives

            // TODO: Consider starting a heartbeat task here if needed, 
            // or handle client removal on stream drop/error.

            // Respond with the SSE stream
            Ok(stream.respond_to(&req)?)
        },
        Err(e) => {
            error!("MCP SSE: Failed to register client {}: {:?}", client_id, e);
            // Return an appropriate HTTP error response
            Ok(HttpResponse::InternalServerError().json(e))
        }
    }
}

/// Configures the SSE routes for an Actix-web application.
///
/// Adds the `/mcp/events/{client_id}` route for establishing SSE connections.
/// It also injects the `McpSseState` into the application data.
///
/// # Arguments
///
/// * `cfg` - The `web::ServiceConfig` to modify.
/// * `sse_state` - The shared `McpSseState` instance.
pub fn configure_sse_service(cfg: &mut web::ServiceConfig, sse_state: Data<McpSseState>) {
    info!("Configuring MCP SSE service endpoint (/mcp/events/{{client_id}})...");
    cfg.app_data(sse_state.clone()) // Add state to app data
        .service(
            web::resource("/mcp/events/{client_id}")
                .route(web::get().to(sse_connection_handler))
        );
    info!("MCP SSE service configured.");
}

pub async fn mcp_sse_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: Data<McpSseState>,
) -> impl Responder {
    let client_id = Uuid::new_v4();
    info!("New MCP SSE client connected: {}", client_id);

    let rx = match state.add_client(client_id).await {
        Ok(rx) => rx,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to add MCP SSE client: {}", e));
        }
    };

    let stream = rx.map(|event| Ok::<_, actix_web::Error>(event));

    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .streaming(stream)
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    info!("Configuring MCP SSE service endpoint (/mcp/events)...");
    cfg.service(
        web::resource("/mcp/events")
            .route(web::get().to(mcp_sse_handler))
    );
}

/// (Potentially Legacy) SSE Server structure using tokio::broadcast.
struct McpSseServer {
    port_state: Arc<McpPortState>,
    event_tx: broadcast::Sender<Value>,
}

impl McpSseServer {
    pub fn new(port_state: Arc<McpPortState>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self { port_state, event_tx }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let port_state = self.port_state.clone();
        let event_tx = self.event_tx.clone();

        HttpServer::new(move || {
            App::new()
                .app_data(Data::new(port_state.clone()))
                .app_data(Data::new(event_tx.subscribe())) // Pass receiver
                .route("/events", web::get().to(handle_sse)) // Generic /events endpoint
        })
        .bind("127.0.0.1:8081")? // Use different port to avoid conflict
        .run()
        .await
    }
}

/// (Potentially Legacy) Actix handler using tokio::broadcast receiver.
async fn handle_sse(
    _port_state: Data<Arc<McpPortState>>, // State not used here
    event_rx: Data<broadcast::Receiver<Value>>,
    req: HttpRequest,
) -> Result<HttpResponse, ActixError> {
    let mut rx = event_rx.get_ref().resubscribe(); // Resubscribe for each connection

    let stream = Sse::from_custom_stream(
        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(value) => {
                        // Convert Value to sse::Event (e.g., using json serialization)
                        if let Ok(data_str) = serde_json::to_string(&value) {
                             yield Event::Data(data_str.into());
                        } else {
                            error!("Failed to serialize broadcast value to JSON string");
                            // Optionally send an error event or skip
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("SSE stream lagged, skipped {} messages", skipped);
                        // Optionally send a notification about lag
                        yield Event::Comment(format!("Stream lagged, skipped {}", skipped).into());
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Broadcast channel closed, ending SSE stream.");
                        break;
                    }
                }
            }
        }
    ).with_keep_alive(Duration::from_secs(15));

    Ok(stream.respond_to(&req)?)
}

/// (Potentially Legacy) Spawns the broadcast-based SSE server.
pub async fn spawn_mcp_sse_server(port_state: Arc<TokioMutex<McpPortState>>) -> std::io::Result<()> {
    // Ensure McpPortState is wrapped correctly if needed by McpSseServer::new
    let server = McpSseServer::new(port_state); 
    server.run().await
} 