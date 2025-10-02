use actix_web::{web, HttpRequest, HttpResponse, Error as ActixError};
use actix_web::http::header::{HeaderValue, ACCEPT, ORIGIN};
use futures::stream::Stream;
use futures::StreamExt;
use log::{info, error, debug, warn};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use uuid::Uuid;
use actix_web::web::Bytes;

use crate::dashboard::services::DashboardState;

// Global state to manage SSE connections and their message queues
lazy_static::lazy_static! {
    static ref SSE_SESSIONS: Arc<RwLock<HashMap<String, mpsc::Sender<String>>>> =
        Arc::new(RwLock::new(HashMap::new()));
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
async fn handle_mcp_request(request: Value, state: web::Data<DashboardState>) -> Value {
    let method = request.get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let params = request.get("params").cloned().unwrap_or(json!({}));
    let request_id = request.get("id").cloned();

    match method {
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
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "tools": [
                        {
                            "name": "list_folders",
                            "description": "List all email folders",
                            "inputSchema": {
                                "type": "object",
                                "properties": {},
                                "required": []
                            }
                        },
                        {
                            "name": "search_emails",
                            "description": "Search emails with various criteria",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "folder": {
                                        "type": "string",
                                        "default": "INBOX"
                                    },
                                    "search": {
                                        "type": "string",
                                        "default": "ALL"
                                    },
                                    "limit": {
                                        "type": "integer",
                                        "default": 10
                                    }
                                },
                                "required": []
                            }
                        },
                        {
                            "name": "fetch_emails_with_mime",
                            "description": "Fetch emails with MIME content",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "folder": {
                                        "type": "string",
                                        "default": "INBOX"
                                    },
                                    "message_ids": {
                                        "type": "array",
                                        "items": {"type": "integer"}
                                    }
                                },
                                "required": ["message_ids"]
                            }
                        }
                    ]
                }
            })
        },
        "tools/call" => {
            // Handle tool calls
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let tool_params = params.get("arguments").cloned().unwrap_or(json!({}));

            // Call the actual MCP handler through the dashboard API
            let mcp_request = json!({
                "tool": tool_name,
                "parameters": tool_params
            });

            match crate::dashboard::api::handlers::execute_mcp_tool(
                state.clone(),
                web::Json(mcp_request)
            ).await {
                Ok(_resp) => {
                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": format!("Tool {} executed successfully", tool_name)
                            }]
                        }
                    })
                },
                Err(e) => {
                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32603,
                            "message": e.to_string()
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
    }
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
    }

    // Process the JSON-RPC request
    let request = body.into_inner();
    let response = handle_mcp_request(request.clone(), state).await;

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
/// Opens an SSE stream for server-initiated messages
pub async fn mcp_get_handler(
    req: HttpRequest,
    state: web::Data<DashboardState>,
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

    info!("Creating SSE stream for session: {}", session_id);

    // Create channel for SSE messages
    let (sender, receiver) = mpsc::channel(100);

    // Store the sender in our sessions map
    {
        let mut sessions = SSE_SESSIONS.write().await;
        sessions.insert(session_id.clone(), sender.clone());
    }

    // Send initial connection message
    let initial_msg = format!(": connected {}\n\n", session_id);
    if let Err(e) = sender.send(initial_msg).await {
        error!("Failed to send initial SSE message: {}", e);
        return Ok(HttpResponse::InternalServerError().finish());
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
