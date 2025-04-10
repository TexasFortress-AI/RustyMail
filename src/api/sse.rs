//! Provides generic Server-Sent Events (SSE) capabilities.
//! 
//! NOTE: This module appears to overlap significantly with `src/api/mcp_sse.rs`.
//! It defines a similar `SseState` and handler (`sse_handler`) for managing SSE clients.
//! However, `mcp_sse.rs` seems more specifically tailored for handling events related
//! to MCP/IMAP operations via the `/mcp/events/{client_id}` endpoint.
//! This module might be intended for more general server broadcasts (like the example
//! `broadcast_task`) or could be partially legacy. Clarification on the intended
//! distinct roles of `sse.rs` and `mcp_sse.rs` might be needed.

use actix_web::{
    web::{self, Data, Path},
    Error as ActixError, HttpRequest, HttpResponse, Responder,
};
use actix_web_lab::sse::{self, Sse, Event};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tokio::{
    sync::{
        Mutex as TokioMutex,
        RwLock,
        mpsc,
    },
    time::{Duration, interval},
};
use std::{
    sync::Arc,
    collections::HashMap,
};
use uuid::Uuid;
use crate::{
    mcp::{
        McpHandler, McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError,
    },
};
use log::{error, info, warn};
use serde::Serialize;
use serde_json;

#[derive(Debug, Serialize)]
struct SseCommandPayload {
    command: String,
    payload: JsonRpcRequest,
}

#[derive(Debug)]
pub struct SseClientInfo {
    sender: mpsc::Sender<sse::Event>,
}

pub struct SseClient {
    sender: mpsc::Sender<sse::Event>,
    info: SseClientInfo,
}

#[derive(Clone)]
pub struct SseState {
    clients: Arc<RwLock<HashMap<Uuid, mpsc::Sender<sse::Event>>>>,
    mcp_handler: Arc<dyn McpHandler>,
    port_state: Arc<TokioMutex<McpPortState>>,
}

impl SseState {
    pub fn new(mcp_handler: Arc<dyn McpHandler>, port_state: Arc<TokioMutex<McpPortState>>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            mcp_handler,
            port_state,
        }
    }

    async fn add_client(&self, client_id: Uuid) -> Result<mpsc::Receiver<sse::Event>, JsonRpcError> {
        let (tx, rx) = mpsc::channel(100);
        let mut clients_guard = self.clients.write().await;
        if clients_guard.contains_key(&client_id) {
            error!("SSE State: Client {} already exists!", client_id);
            return Err(JsonRpcError::internal_error(format!("Client ID {} already exists", client_id)));
        }
        clients_guard.insert(client_id, tx);
        info!("SSE State: Client {} added.", client_id);
        Ok(rx)
    }

    async fn remove_client(&self, client_id: &Uuid) {
        if self.clients.write().await.remove(client_id).is_some() {
            info!("SSE State: Client {} removed.", client_id);
        } else {
            warn!("SSE State: Attempted to remove non-existent client {}.", client_id);
        }
    }

    pub async fn broadcast_event(&self, event: sse::Event) {
        let clients = self.clients.read().await;
        if clients.is_empty() {
            warn!("SSE State: No clients connected, cannot broadcast event: {:?}", event);
            return;
        }
        info!("SSE State: Broadcasting event to {} clients: {:?}", clients.len(), event);
        let sends = clients.iter().map(|(_, tx)| {
            let event_clone = event.clone();
            async move {
                if let Err(e) = tx.send(event_clone).await {
                    error!("SSE State: Failed to send event to client: {}", e);
                }
            }
        });
        futures_util::future::join_all(sends).await;
    }

    async fn handle_mcp_request(&self, client_id: &Uuid, request: JsonRpcRequest) {
        warn!("SSE State: Handling MCP request for client {} and broadcasting response to ALL clients.", client_id);
        let request_json = match serde_json::to_value(request.clone()) {
            Ok(v) => v,
            Err(e) => {
                error!("SSE State: Failed to serialize MCP request to JSON: {}", e);
                let err_event = Event::Data(sse::Data::new(r#"{"type":"error", "message":"Failed to serialize request"}"#));
                self.broadcast_event(err_event).await;
                return;
            }
        };
        
        let response_json = self.mcp_handler.handle_request(self.port_state.clone(), request_json).await;
        
        match serde_json::to_string(&response_json) {
            Ok(response_str) => {
                info!("SSE State: Broadcasting MCP response: {}", response_str);
                self.broadcast_event(Event::Data(sse::Data::new(response_str))).await;
            }
            Err(e) => {
                 error!("SSE State: Failed to serialize MCP response for broadcast: {}", e);
                 let err_event = Event::Data(sse::Data::new(&format!(r#"{{"type":"error", "message":"Failed to serialize response: {}"}}"#, e)));
                 self.broadcast_event(err_event).await;
            }
        }
    }
}

pub async fn sse_handler(
    req: HttpRequest,
    _stream: web::Payload,
    state: Data<SseState>,
) -> Result<HttpResponse, ActixError> {
    let client_id = Uuid::new_v4();
    info!("SSE State: New client connecting, assigning ID: {}", client_id);

    let rx = match state.add_client(client_id).await {
        Ok(rx) => rx,
        Err(e) => {
            error!("SSE State: Failed to add client {}: {:?}", client_id, e);
            return Ok(HttpResponse::InternalServerError().json(e));
        }
    };

    let sse_stream = Sse::from_custom_stream(ReceiverStream::new(rx))
         .with_keep_alive(Duration::from_secs(20));
         
    info!("SSE State: Client {} connected, returning stream.", client_id);
    
    Ok(sse_stream.respond_to(&req)?)
}

async fn broadcast_task(sse_state: Data<SseState>) {
    info!("Starting SSE broadcast_task (sending 'tick' every 5s)");
    let mut interval = interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let event = Event::Comment("tick".into());
        sse_state.broadcast_event(event).await;
    }
}

pub fn configure_sse_service(cfg: &mut web::ServiceConfig, sse_state: Data<SseState>) {
    info!("Configuring generic SSE service endpoint (/events)...");
    cfg.app_data(sse_state.clone())
       .service(
        web::resource("/events")
            .route(web::get().to(sse_handler))
    );
    info!("Generic SSE service configured at /events.");
} 