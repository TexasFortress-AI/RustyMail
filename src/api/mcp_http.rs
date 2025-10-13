use actix_web::{web, HttpRequest, HttpResponse, Error as ActixError};
use actix_web::http::header::{ACCEPT, ORIGIN};
use futures::stream::Stream;
use futures::StreamExt;
use log::{info, error, debug, warn};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::{HashMap, VecDeque};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use uuid::Uuid;
use actix_web::web::Bytes;

use crate::dashboard::services::DashboardState;

const SESSION_TIMEOUT: Duration = Duration::from_secs(600); // 10 minutes
const EVENT_HISTORY_SIZE: usize = 100;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60); // 1 minute

/// Session data with event history for resumability
struct SessionData {
    sender: mpsc::Sender<String>,
    last_activity: Instant,
    event_history: VecDeque<(u64, String)>,
    next_event_id: u64,
}

impl SessionData {
    fn new(sender: mpsc::Sender<String>) -> Self {
        Self {
            sender,
            last_activity: Instant::now(),
            event_history: VecDeque::with_capacity(EVENT_HISTORY_SIZE),
            next_event_id: 1,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    fn is_expired(&self) -> bool {
        self.last_activity.elapsed() > SESSION_TIMEOUT
    }

    async fn send_event(&mut self, data: String) -> Result<(), String> {
        let event_id = self.next_event_id;
        self.next_event_id += 1;

        // Format as SSE with event ID
        let message = format!("id: {}\ndata: {}\n\n", event_id, data);

        // Store in history
        self.event_history.push_back((event_id, data.clone()));
        if self.event_history.len() > EVENT_HISTORY_SIZE {
            self.event_history.pop_front();
        }

        self.sender.send(message).await
            .map_err(|e| e.to_string())?;

        self.update_activity();
        Ok(())
    }

    fn get_events_since(&self, last_event_id: u64) -> Vec<String> {
        self.event_history
            .iter()
            .filter(|(id, _)| *id > last_event_id)
            .map(|(id, data)| format!("id: {}\ndata: {}\n\n", id, data))
            .collect()
    }
}

// Global state to manage SSE connections and their message queues
lazy_static::lazy_static! {
    static ref SSE_SESSIONS: Arc<RwLock<HashMap<String, SessionData>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

// Start background cleanup task
pub fn start_session_cleanup() {
    tokio::spawn(async {
        let mut cleanup_interval = interval(CLEANUP_INTERVAL);
        loop {
            cleanup_interval.tick().await;
            cleanup_expired_sessions().await;
        }
    });
}

async fn cleanup_expired_sessions() {
    let mut sessions = SSE_SESSIONS.write().await;
    let initial_count = sessions.len();

    sessions.retain(|session_id, session| {
        if session.is_expired() {
            info!("Cleaning up expired session: {}", session_id);
            false
        } else {
            true
        }
    });

    let removed = initial_count - sessions.len();
    if removed > 0 {
        info!("Cleaned up {} expired sessions", removed);
    }
}

/// SSE stream implementation for Streamable HTTP transport
struct McpSseStream {
    receiver: mpsc::Receiver<String>,
    heartbeat: IntervalStream,
}

impl Stream for McpSseStream {
    type Item = Result<Bytes, ActixError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for messages first
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(msg)) => {
                cx.waker().wake_by_ref();
                return Poll::Ready(Some(Ok(Bytes::from(msg))));
            }
            Poll::Ready(None) => {
                error!("MCP SSE receiver channel closed, terminating stream");
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        // Send heartbeat if no messages
        if let Poll::Ready(Some(_)) = self.heartbeat.poll_next_unpin(cx) {
            let heartbeat = format!(": heartbeat\n\n");
            cx.waker().wake_by_ref();
            return Poll::Ready(Some(Ok(Bytes::from(heartbeat))));
        }

        Poll::Pending
    }
}

/// Validate Origin header to prevent DNS rebinding attacks
fn validate_origin(req: &HttpRequest) -> bool {
    if let Some(origin) = req.headers().get(ORIGIN) {
        if let Ok(origin_str) = origin.to_str() {
            // Allow localhost and 127.0.0.1
            if origin_str.contains("localhost") || origin_str.contains("127.0.0.1") {
                return true;
            }
            warn!("Rejected request from origin: {}", origin_str);
            return false;
        }
    }
    // Allow requests without Origin header (e.g., from non-browser clients)
    true
}

/// Handle MCP request and generate JSON-RPC response
/// Returns None for notifications (requests without id), Some(Value) for requests
async fn handle_mcp_request(request: Value, state: web::Data<DashboardState>) -> Option<Value> {
    let method = request.get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let params = request.get("params").cloned().unwrap_or(json!({}));
    let request_id = request.get("id").cloned();

    // Check if this is a notification (no id field)
    // Notifications should not receive responses per JSON-RPC 2.0 spec
    let is_notification = request_id.is_none();

    // Handle notifications - return None (no response) per JSON-RPC 2.0 spec
    if is_notification {
        match method {
            "notifications/initialized" => {
                debug!("Received notifications/initialized - no response per spec");
                return None;
            },
            _ => {
                debug!("Received unknown notification: {} - no response per spec", method);
                return None;
            }
        }
    }

    // Handle requests - return Some(response)
    let response = match method {
        "initialize" => {
            // Generate session ID for this client
            let session_id = Uuid::new_v4().to_string();

            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "rustymail-mcp",
                        "version": "1.0.0"
                    },
                    "_meta": {
                        "sessionId": session_id
                    }
                }
            })
        },
        "tools/list" => {
            // Get all tools in MCP JSON-RPC format from dashboard handlers
            let tools = crate::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();

            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "tools": tools
                }
            })
        },
        "tools/call" => {
            // Handle tool calls
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let tool_params = params.get("arguments").cloned().unwrap_or(json!({}));

            // Call the tool execution logic directly to get the result
            let result = crate::dashboard::api::handlers::execute_mcp_tool_inner(
                state.get_ref(),
                tool_name,
                tool_params
            ).await;

            // Format result for MCP protocol
            match result.get("success").and_then(|v| v.as_bool()) {
                Some(true) => {
                    // Success - format data as MCP content
                    let data = result.get("data").cloned().unwrap_or(json!(null));
                    let data_str = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "null".to_string());

                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": data_str
                            }]
                        }
                    })
                },
                Some(false) | None => {
                    // Error - extract error message
                    let error_msg = result.get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Tool execution failed");

                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32603,
                            "message": error_msg.to_string()
                        }
                    })
                }
            }
        },
        _ => {
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            })
        }
    };

    Some(response)
}

/// POST handler for MCP endpoint
/// Handles JSON-RPC requests and returns responses
pub async fn mcp_post_handler(
    req: HttpRequest,
    body: web::Json<Value>,
    state: web::Data<DashboardState>,
) -> Result<HttpResponse, ActixError> {
    info!("MCP POST request received");

    // Validate Origin header for security
    if !validate_origin(&req) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32600,
                "message": "Invalid origin"
            }
        })));
    }

    // Check Accept header
    let accept_header = req.headers().get(ACCEPT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/json");

    debug!("Accept header: {}", accept_header);

    // Extract session ID if present
    let session_id = req.headers().get("Mcp-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref sid) = session_id {
        debug!("Request with session ID: {}", sid);

        // Update session activity
        let mut sessions = SSE_SESSIONS.write().await;
        if let Some(session) = sessions.get_mut(sid) {
            session.update_activity();
        }
    }

    // Process the JSON-RPC request
    let request = body.into_inner();
    let response_opt = handle_mcp_request(request.clone(), state).await;

    // If this is a notification, don't send a response
    let response = match response_opt {
        Some(r) => r,
        None => {
            // Notification - return 204 No Content
            return Ok(HttpResponse::NoContent().finish());
        }
    };

    // Check if this is an initialize response with session ID
    let mut response_builder = HttpResponse::Ok();

    if let Some(meta) = response.get("result").and_then(|r| r.get("_meta")) {
        if let Some(session_id) = meta.get("sessionId").and_then(|s| s.as_str()) {
            response_builder.insert_header(("Mcp-Session-Id", session_id));
        }
    }

    // Return response based on Accept header
    if accept_header.contains("text/event-stream") {
        // Client wants SSE format
        let sse_data = format!("data: {}\n\n", serde_json::to_string(&response).unwrap());
        Ok(response_builder
            .content_type("text/event-stream")
            .insert_header(("Cache-Control", "no-cache"))
            .body(sse_data))
    } else {
        // Client wants JSON format
        Ok(response_builder
            .content_type("application/json")
            .json(response))
    }
}

/// GET handler for MCP endpoint
/// Opens an SSE stream for server-initiated messages with resumability support
pub async fn mcp_get_handler(
    req: HttpRequest,
    _state: web::Data<DashboardState>,
) -> Result<HttpResponse, ActixError> {
    info!("MCP GET request received for SSE stream");

    // Validate Origin header
    if !validate_origin(&req) {
        return Ok(HttpResponse::Forbidden().finish());
    }

    // Check Accept header - must request text/event-stream
    let accept_header = req.headers().get(ACCEPT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if !accept_header.contains("text/event-stream") {
        return Ok(HttpResponse::MethodNotAllowed()
            .insert_header(("Allow", "POST"))
            .finish());
    }

    // Extract or create session ID
    let session_id = req.headers().get("Mcp-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Check for Last-Event-ID for connection resumption
    let last_event_id = req.headers().get("Last-Event-ID")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    info!("Creating SSE stream for session: {} (last_event_id: {:?})", session_id, last_event_id);

    // Create channel for SSE messages
    let (sender, receiver) = mpsc::channel(100);

    // Check if this is a reconnection
    let mut missed_events = Vec::new();
    {
        let mut sessions = SSE_SESSIONS.write().await;

        if let Some(existing_session) = sessions.get_mut(&session_id) {
            // Resuming existing session
            info!("Resuming existing session: {}", session_id);
            existing_session.update_activity();

            // Get missed events if Last-Event-ID provided
            if let Some(last_id) = last_event_id {
                missed_events = existing_session.get_events_since(last_id);
                info!("Found {} missed events since ID {}", missed_events.len(), last_id);
            }

            // Update sender for new connection
            existing_session.sender = sender.clone();
        } else {
            // New session
            info!("Creating new session: {}", session_id);
            sessions.insert(session_id.clone(), SessionData::new(sender.clone()));
        }
    }

    // Send initial connection message
    let initial_msg = if last_event_id.is_some() {
        format!(": reconnected {}\n\n", session_id)
    } else {
        format!(": connected {}\n\n", session_id)
    };

    if let Err(e) = sender.send(initial_msg).await {
        error!("Failed to send initial SSE message: {}", e);
        return Ok(HttpResponse::InternalServerError().finish());
    }

    // Send missed events for reconnection
    for event in missed_events {
        if let Err(e) = sender.send(event).await {
            error!("Failed to send missed event: {}", e);
        }
    }

    // Create the SSE stream
    let stream = McpSseStream {
        receiver,
        heartbeat: IntervalStream::new(interval(Duration::from_secs(30))),
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Mcp-Session-Id", session_id))
        .streaming(stream))
}

/// Configure MCP Streamable HTTP routes
pub fn configure_mcp_routes(cfg: &mut web::ServiceConfig) {
    info!("Configuring MCP Streamable HTTP transport routes");

    // Main MCP endpoint supporting both GET and POST
    cfg.service(
        web::resource("/mcp")
            .route(web::post().to(mcp_post_handler))
            .route(web::get().to(mcp_get_handler))
    );

    // API versioned endpoint
    cfg.service(
        web::resource("/mcp/v1")
            .route(web::post().to(mcp_post_handler))
            .route(web::get().to(mcp_get_handler))
    );
}
