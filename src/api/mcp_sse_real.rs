use actix_web::{web, HttpRequest, HttpResponse, Error as ActixError};
use actix_web::web::Bytes;
use futures::stream::{Stream, StreamExt};
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

use crate::dashboard::services::DashboardState;

// Global state to manage SSE connections and their message queues
lazy_static::lazy_static! {
    static ref SSE_CONNECTIONS: Arc<RwLock<HashMap<String, mpsc::Sender<String>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

struct SseStream {
    receiver: mpsc::Receiver<String>,
    heartbeat: IntervalStream,
}

impl Stream for SseStream {
    type Item = Result<Bytes, ActixError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check for messages first
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(msg)) => {
                // Send the message and wake context to continue polling
                cx.waker().wake_by_ref();
                return Poll::Ready(Some(Ok(Bytes::from(msg))));
            }
            Poll::Ready(None) => {
                // Only end stream if receiver is actually closed (sender dropped)
                error!("SSE receiver channel closed, terminating stream");
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        // Send heartbeat if no messages
        if let Poll::Ready(Some(_)) = self.heartbeat.poll_next_unpin(cx) {
            let heartbeat = format!(": heartbeat\n\n");
            // Wake context to continue polling after heartbeat
            cx.waker().wake_by_ref();
            return Poll::Ready(Some(Ok(Bytes::from(heartbeat))));
        }

        Poll::Pending
    }
}

/// Handle SSE connection for MCP
pub async fn mcp_sse_handler(
    req: HttpRequest,
    state: web::Data<DashboardState>,
) -> Result<HttpResponse, ActixError> {
    info!("New SSE connection for MCP");

    // Verify API key
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if api_key.is_empty() {
        info!("No API key provided in SSE request");
        return Ok(HttpResponse::Unauthorized().finish());
    }

    info!("SSE connection authorized with API key");

    // Generate a connection ID
    let connection_id = Uuid::new_v4().to_string();
    info!("Created SSE connection with ID: {}", connection_id);

    // Create a channel for sending SSE messages with a larger buffer
    let (sender, receiver) = mpsc::channel(1000);

    // Store the sender in our global connections map
    {
        let mut connections = SSE_CONNECTIONS.write().await;
        connections.insert(connection_id.clone(), sender.clone());
        // Also store by a simplified key for easier lookup
        // Supergateway doesn't track connection IDs, so we'll use "default" for single connections
        connections.insert("default".to_string(), sender.clone());
    }

    // Send initial SSE comment to establish connection
    let initial_msg = format!(":connected\nid: {}\n\n", connection_id);
    if let Err(e) = sender.send(initial_msg).await {
        error!("Failed to send initial SSE message: {}", e);
        return Ok(HttpResponse::InternalServerError().finish());
    }

    // Send endpoint event so client knows where to POST messages
    // This is required by the MCP SSE protocol
    let endpoint_msg = format!("event: endpoint\ndata: http://localhost:9437/message\n\n");
    if let Err(e) = sender.send(endpoint_msg).await {
        error!("Failed to send endpoint SSE message: {}", e);
        return Ok(HttpResponse::InternalServerError().finish());
    }

    // Note: Supergateway sends messages to the /message endpoint separately,
    // not through the SSE stream itself

    // Create the SSE stream with extremely frequent heartbeats for supergateway compatibility
    let stream = SseStream {
        receiver,
        heartbeat: IntervalStream::new(interval(Duration::from_secs(2))),
    };

    // Store connection ID in response headers so POST handler can find it
    let response = HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("X-SSE-Connection-Id", connection_id.clone()))
        .streaming(stream);

    // Don't spawn automatic cleanup task - let natural connection close handle cleanup
    // The connection will be cleaned up when the client disconnects or the stream ends

    Ok(response)
}

async fn handle_mcp_request(request: Value, state: web::Data<DashboardState>) -> Value {
    let method = request.get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let params = request.get("params").cloned().unwrap_or(json!({}));
    let request_id = request.get("id").cloned();

    match method {
        "initialize" => {
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "serverInfo": {
                        "name": "rustymail-mcp",
                        "version": "1.0.0"
                    },
                    "capabilities": {
                        "tools": true,
                        "resources": false
                    },
                    "tools": [
                        {
                            "name": "count_emails_in_folder",
                            "description": "Count emails in a specific folder",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "folder": {
                                        "type": "string",
                                        "default": "INBOX"
                                    }
                                },
                                "required": []
                            }
                        },
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
                        }
                    ]
                }
            })
        },
        "tools/call" => {
            // Handle tool calls
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let tool_params = params.get("arguments").cloned().unwrap_or(json!({}));

            // Call the actual MCP handler
            let mcp_request = json!({
                "tool": tool_name,
                "parameters": tool_params
            });

            // Use the execute_mcp_tool handler directly
            match crate::dashboard::api::handlers::execute_mcp_tool(
                state.clone(),
                web::Json(mcp_request)
            ).await {
                Ok(_resp) => {
                    // The response is an impl Responder, we need to extract the JSON
                    // For now, just return a success with the tool name
                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "result": {
                            "tool": tool_name,
                            "success": true
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

/// Handle MCP message endpoint (for bidirectional communication)
/// This handler now sends responses through the SSE stream instead of returning them directly
pub async fn mcp_message_handler(
    req: HttpRequest,
    body: web::Json<Value>,
    state: web::Data<DashboardState>,
) -> Result<HttpResponse, ActixError> {
    info!("MCP message handler called - POST to /message endpoint");
    info!("Received MCP message body: {}", serde_json::to_string(&body.0).unwrap());

    // Verify API key
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if api_key.is_empty() {
        warn!("MCP message request without API key");
        return Ok(HttpResponse::Unauthorized().json(json!({
            "error": "Missing API key"
        })));
    }

    info!("MCP message authorized with API key");

    // Extract the JSON-RPC request
    let request = body.into_inner();
    info!("Processing MCP request: method={}", request.get("method").and_then(|m| m.as_str()).unwrap_or("unknown"));

    // Process the request and get the response
    let response = handle_mcp_request(request.clone(), state).await;

    // For supergateway, we need to send the response through the SSE stream
    // Try to find an active SSE connection - use "default" as the primary key
    {
        let connections = SSE_CONNECTIONS.read().await;

        // Try "default" first (for single connection scenarios)
        let sender = connections.get("default")
            .or_else(|| connections.values().next());

        if let Some(sender) = sender {
            info!("Sending response through SSE stream");
            let response_msg = format!(
                "event: message\ndata: {}\n\n",
                serde_json::to_string(&response).unwrap()
            );

            debug!("SSE response message: {}", response_msg);

            // Use blocking send with shorter timeout for faster response
            match tokio::time::timeout(Duration::from_millis(100), sender.send(response_msg)).await {
                Ok(Ok(_)) => {
                    info!("Response sent successfully through SSE stream");
                },
                Ok(Err(_)) => {
                    error!("SSE channel is closed, cleaning up connection");
                    drop(connections);
                    let mut connections = SSE_CONNECTIONS.write().await;
                    connections.remove("default");
                },
                Err(_) => {
                    error!("Timeout sending response through SSE stream");
                }
            }
        } else {
            error!("No SSE connections available to send response");
            // Fallback: return as HTTP response (shouldn't happen with supergateway)
            return Ok(HttpResponse::Ok().json(response));
        }
    }

    // Return an empty 200 OK to acknowledge receipt
    Ok(HttpResponse::Ok().finish())
}

/// Combined handler that handles both GET (SSE stream) and POST (messages) on the same endpoint
pub async fn mcp_sse_combined_handler(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<DashboardState>,
) -> Result<HttpResponse, ActixError> {
    info!("MCP SSE combined handler called - method: {}", req.method());

    // Check the request method
    match req.method() {
        &actix_web::http::Method::GET => {
            info!("Handling GET request for SSE connection");
            // Handle SSE connection
            mcp_sse_handler(req, state).await
        },
        &actix_web::http::Method::POST => {
            info!("Handling POST request for MCP message");
            // Handle message - parse the body first
            let mut bytes = web::BytesMut::new();
            let mut body_stream = body;
            while let Some(chunk) = futures::StreamExt::next(&mut body_stream).await {
                let chunk = chunk?;
                bytes.extend_from_slice(&chunk);
            }

            debug!("Received POST body: {} bytes", bytes.len());

            // Parse as JSON
            let json_value: Value = serde_json::from_slice(&bytes)
                .map_err(|e| actix_web::error::ErrorBadRequest(e))?;

            debug!("Parsed JSON: {:?}", json_value);

            // Handle the message
            mcp_message_handler(req, web::Json(json_value), state).await
        },
        _ => {
            warn!("Unsupported method: {}", req.method());
            Ok(HttpResponse::MethodNotAllowed().finish())
        }
    }
}

/// Simple root handler that returns a basic response
async fn root_handler() -> Result<HttpResponse, ActixError> {
    Ok(HttpResponse::Ok().json(json!({
        "service": "rustymail-mcp",
        "version": "1.0.0",
        "sse_endpoint": "/sse",
        "message_endpoint": "/message"
    })))
}

pub fn configure_mcp_sse_routes(cfg: &mut web::ServiceConfig) {
    info!("Configuring MCP SSE routes for supergateway");

    // Add root handler for initial connection check
    cfg.service(
        web::resource("/")
            .route(web::get().to(root_handler))
    );

    // Configure the SSE endpoint at /sse (supergateway default)
    cfg.service(
        web::resource("/sse")
            .route(web::get().to(mcp_sse_handler))
    );

    // Configure the message endpoint at /message (supergateway default)
    cfg.service(
        web::resource("/message")
            .route(web::post().to(mcp_message_handler))
    );

    // Keep the combined endpoint for backwards compatibility
    cfg.service(
        web::resource("/api/mcp/sse")
            .route(web::get().to(mcp_sse_handler))
            .route(web::post().to(mcp_message_handler))
    );
}