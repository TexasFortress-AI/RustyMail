//! Provides generic Server-Sent Events (SSE) capabilities.
//! 
//! NOTE: This module appears to overlap significantly with `src/api/mcp_sse.rs`.
//! It defines a similar `SseState` and handler (`sse_handler`) for managing SSE clients.
//! However, `mcp_sse.rs` seems more specifically tailored for handling events related
//! to MCP/IMAP operations via the `/mcp/events/{client_id}` endpoint.
//! This module might be intended for more general server broadcasts (like the example
//! `broadcast_task`) or could be partially legacy. Clarification on the intended
//! distinct roles of `sse.rs` and `mcp_sse.rs` might be needed.

use actix::prelude::*;
use actix::Context as ActorContext;
use actix_web::{
    web::{self, Data, Payload},
    Error as ActixError, HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use actix_web_lab::sse::{self, Sse};
use futures_util::{StreamExt as _, TryStreamExt as _};
use log::{debug, info, error, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Mutex as TokioMutex},
    time::interval,
};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{
    config::Settings,
    mcp::{
        handler::McpHandler,
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError},
        ErrorCode,
    },
    session_manager::SessionManager,
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct ClientState {
    hb: Instant,
    session_id: String,
}

impl ClientState {
    fn new(session_id: String) -> Self {
        Self { hb: Instant::now(), session_id }
    }
}

pub struct SseState {
    sessions: HashMap<String, mpsc::Sender<sse::Event>>,
    hb_interval: Duration,
    client_timeout: Duration,
    mcp_handler: Arc<dyn McpHandler>,
    port_state: Arc<TokioMutex<McpPortState>>,
}

impl std::fmt::Debug for SseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseState")
            .field("sessions", &self.sessions.len())
            .field("hb_interval", &self.hb_interval)
            .field("client_timeout", &self.client_timeout)
            .field("has_mcp_handler", &true)
            .field("has_port_state", &true)
            .finish()
    }
}

impl SseState {
    pub fn new(mcp_handler: Arc<dyn McpHandler>, port_state: Arc<TokioMutex<McpPortState>>) -> Self {
        SseState {
            sessions: HashMap::new(),
            hb_interval: HEARTBEAT_INTERVAL,
            client_timeout: CLIENT_TIMEOUT,
            mcp_handler,
            port_state,
        }
    }

    fn heartbeat(&mut self, ctx: &mut ActorContext<Self>) {
        ctx.run_interval(self.hb_interval, |act, ctx_inner| {
            let mut dead_sessions = Vec::new();
            for (id, client_sender) in &act.sessions {
                if client_sender.is_closed() {
                    warn!("SSE session {} disconnected. Removing.", id);
                    dead_sessions.push(id.clone());
                }
            }

            for id in dead_sessions {
                act.sessions.remove(&id);
            }
        });
    }

    fn add_session(&mut self, id: String, sender: mpsc::Sender<sse::Event>) {
        info!("Adding new SSE session: {}", id);
        self.sessions.insert(id, sender);
    }

    fn remove_session(&mut self, id: &str) {
        info!("Removing SSE session: {}", id);
        self.sessions.remove(id);
    }

    async fn broadcast(&self, msg: String) {
        let event = sse::Event::Data(sse::Data::new(msg.clone()));
        for sender in self.sessions.values() {
            if let Err(e) = sender.send(event.clone()).await {
                error!("Failed to broadcast message: {:?}", e);
            }
        }
    }

    async fn handle_mcp_request(&self, session_id: &str, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        debug!("Handling MCP request from SSE session {}: {:?}", session_id, request);
        let request_json = match serde_json::to_value(request) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to serialize request to JSON: {}", e);
                return Some(JsonRpcResponse::error(None, JsonRpcError::server_error(ErrorCode::ParseError as i64, e.to_string())));
            }
        };

        let port_state_clone = self.port_state.clone();

        // TODO: Session ID needs to be handled differently - perhaps stored in state
        let response_value = self.mcp_handler.handle_request(port_state_clone, request_json).await;
        match serde_json::from_value(response_value) {
            Ok(resp) => Some(resp),
            Err(e) => {
                error!("Failed to deserialize MCP response: {}", e);
                Some(JsonRpcResponse::error(None, JsonRpcError::server_error(ErrorCode::InternalError as i64, e.to_string())))
            }
        }
    }
}

impl Actor for SseState {
    type Context = ActorContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("SseState actor started");
        self.heartbeat(ctx);
    }
}

pub async fn sse_handler(
    req: HttpRequest,
    state: Data<Arc<TokioMutex<SseState>>>,
) -> Result<HttpResponse, ActixError> {
    info!("Handling new SSE connection request");
    let sse_state = state.get_ref().clone();
    let (tx, rx) = mpsc::channel(100);
    let session_id = Uuid::new_v4().to_string();

    sse_state.lock().await.add_session(session_id.clone(), tx);

    info!("SSE connection established for session {}", session_id);

    use actix_web::web::Bytes;

    let stream = ReceiverStream::new(rx).map(|event| {
        // Convert SSE Event to Bytes for streaming
        let event_str = format!("data: SSE event\n\n");
        Ok::<_, ActixError>(Bytes::from(event_str))
    });

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream))
}

async fn broadcast_update(state: Data<Arc<TokioMutex<SseState>>>, message: &str) {
    let state_guard = state.lock().await;
    state_guard.broadcast(message.to_string()).await;
}

pub fn configure_sse_service(cfg: &mut web::ServiceConfig, sse_state: Data<Arc<TokioMutex<SseState>>>) {
    info!("Configuring generic SSE service endpoint (/events)...");
    cfg.app_data(sse_state.clone())
       .service(
        web::resource("/events")
            .route(web::get().to(sse_handler))
    );
    info!("Generic SSE service configured at /events.");
} 