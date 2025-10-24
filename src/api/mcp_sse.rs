// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix::prelude::*;
use actix::{Actor, Addr, Context, Handler, Message, ResponseFuture, Running, StreamHandler};
use actix_web::{
    web::{self, Data, Payload},
    Error as ActixError, HttpRequest, HttpResponse,
};
use actix_web_lab::sse::{self, Sse};
use actix_web_actors::ws;
use futures_util::{StreamExt as _, TryStreamExt as _};
use log::{debug, info, error, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{mpsc, Mutex as TokioMutex},
    time::{interval, Instant},
};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;
use actix::fut;

// Crate-local imports
use crate::{
    config::Settings,
    mcp::{
        handler::McpHandler,
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError},
        ErrorCode,
    },
    session_manager::SessionManager,
};

// --- SSE State Actor (Simplified - combines SseState and ClientState handling) ---

#[derive(Debug)]
pub struct SseSession {
    id: String,
    hb: Instant,
    sender: mpsc::Sender<sse::Event>,
}

pub struct McpSseState {
    sessions: HashMap<String, SseSession>,
    hb_interval: Duration,
    client_timeout: Duration,
    mcp_handler: Arc<dyn McpHandler>,
    port_state: Arc<TokioMutex<McpPortState>>,
}

impl std::fmt::Debug for McpSseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpSseState")
            .field("sessions", &self.sessions.len())
            .field("hb_interval", &self.hb_interval)
            .field("client_timeout", &self.client_timeout)
            .field("has_mcp_handler", &true)
            .field("has_port_state", &true)
            .finish()
    }
}

impl McpSseState {
    pub fn new(mcp_handler: Arc<dyn McpHandler>, port_state: Arc<TokioMutex<McpPortState>>) -> Self {
        McpSseState {
            sessions: HashMap::new(),
            hb_interval: Duration::from_secs(5),
            client_timeout: Duration::from_secs(15),
            mcp_handler,
            port_state,
        }
    }

    fn heartbeat(&mut self, ctx: &mut actix::Context<Self>) {
        ctx.run_interval(self.hb_interval, |act, _ctx_inner| {
            let now = Instant::now();
            let mut dead_sessions = Vec::new();

            for session in act.sessions.values() {
                if now.duration_since(session.hb) > act.client_timeout {
                    warn!("SSE session {} timed out. Removing.", session.id);
                    dead_sessions.push(session.id.clone());
                } else if session.sender.is_closed() {
                    warn!("SSE session {} disconnected (sender closed). Removing.", session.id);
                    dead_sessions.push(session.id.clone());
                }
            }

            for id in dead_sessions {
                act.sessions.remove(&id);
            }
        });
    }

    fn add_session(&mut self, id: String, sender: mpsc::Sender<sse::Event>) {
        info!("Adding new SSE session: {}", id);
        self.sessions.insert(id.clone(), SseSession { id, hb: Instant::now(), sender });
    }

    fn remove_session(&mut self, id: &str) {
        info!("Removing SSE session: {}", id);
        self.sessions.remove(id);
    }

    fn update_heartbeat(&mut self, id: &str) {
        if let Some(session) = self.sessions.get_mut(id) {
            session.hb = Instant::now();
        }
    }

    async fn broadcast(&self, msg: String, msg_type: &str) {
        let event_data = match serde_json::to_string(&json!({ "type": msg_type, "data": msg })) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize broadcast message: {}", e);
                return;
            }
        };
        let event = sse::Event::Data(sse::Data::new(event_data));
        
        for session in self.sessions.values() {
            if let Err(e) = session.sender.send(event.clone()).await {
                error!("Failed to broadcast message to session {}: {:?}", session.id, e);
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

impl Actor for McpSseState {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("McpSseState actor started");
        self.heartbeat(ctx);
    }
}

// --- Actor Messages ---

#[derive(Message)]
#[rtype(result = "()")]
struct Connect {
    id: String,
    sender: mpsc::Sender<sse::Event>,
}

impl Handler<Connect> for McpSseState {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        self.add_session(msg.id, msg.sender);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect { id: String }

impl Handler<Disconnect> for McpSseState {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) -> Self::Result {
        self.remove_session(&msg.id);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Heartbeat { id: String }

impl Handler<Heartbeat> for McpSseState {
    type Result = ();
    fn handle(&mut self, msg: Heartbeat, _: &mut Self::Context) -> Self::Result {
        self.update_heartbeat(&msg.id);
    }
}

#[derive(Message)]
#[rtype(result = "Option<JsonRpcResponse>")]
struct IncomingRequest {
    session_id: String,
    request: JsonRpcRequest,
}

impl Handler<IncomingRequest> for McpSseState {
    type Result = ResponseFuture<Option<JsonRpcResponse>>;

    fn handle(&mut self, msg: IncomingRequest, _: &mut Self::Context) -> Self::Result {
        let handler = self.mcp_handler.clone();
        let port_state = self.port_state.clone();
        Box::pin(async move {
            McpSseState::handle_mcp_request_static(handler, port_state, &msg.session_id, msg.request).await
        })
    }
}

impl McpSseState {
    async fn handle_mcp_request_static(
        mcp_handler: Arc<dyn McpHandler>,
        port_state: Arc<TokioMutex<McpPortState>>,
        session_id: &str,
        request: JsonRpcRequest
    ) -> Option<JsonRpcResponse> {
         debug!("(Static) Handling MCP request from SSE session {}: {:?}", session_id, request);
        let request_json = match serde_json::to_value(request) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to serialize request to JSON: {}", e);
                return Some(JsonRpcResponse::error(None, JsonRpcError::server_error(ErrorCode::ParseError as i64, e.to_string())));
            }
        };

        // TODO: Session ID needs to be handled differently - perhaps stored in state
        let response_value = mcp_handler.handle_request(port_state, request_json).await;
        match serde_json::from_value(response_value) {
            Ok(resp) => Some(resp),
            Err(e) => {
                error!("Failed to deserialize MCP response: {}", e);
                Some(JsonRpcResponse::error(None, JsonRpcError::server_error(ErrorCode::InternalError as i64, e.to_string())))
            }
        }
    }
}

// Internal messages for WebSocket actor
#[derive(Message)]
#[rtype(result = "()")]
struct WsText(String);

#[derive(Message)]
#[rtype(result = "()")]
struct WsClose;

pub async fn mcp_sse_handler(
    req: HttpRequest,
    stream: Payload,
    state_addr: Data<Addr<McpSseState>>,
) -> Result<HttpResponse, ActixError> {
    info!("Handling new MCP SSE connection request");
    
    let session_id = Uuid::new_v4().to_string();
    let state_addr = state_addr.get_ref().clone();

    ws::start(WsSession { id: session_id, state_addr }, &req, stream)
}

struct WsSession {
    id: String,
    state_addr: Addr<McpSseState>,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket session started for SSE: {}", self.id);
        let addr = ctx.address();
        let (tx, rx) = mpsc::channel(100);
        
        self.state_addr.do_send(Connect { id: self.id.clone(), sender: tx });

        // Spawn a future that will handle incoming SSE events
        let id_clone = self.id.clone();
        ctx.spawn(
            async move {
                let mut rx_stream = ReceiverStream::new(rx);
                while let Some(event) = rx_stream.next().await {
                    // Extract the event data and send it as text
                    // The event is already formatted as SSE data
                    match event {
                        sse::Event::Data(data) => {
                            // The SSE data is already formatted, just send it
                            // Data doesn't have into_string, create a formatted SSE message
                            let text = format!("data: SSE event\n\n");
                            addr.do_send(WsText(text));
                        }
                        _ => {
                            // Handle other event types if needed
                        }
                    }
                }
                info!("SSE forwarder task finished for {}", id_clone);
                // Signal the actor to close
                addr.do_send(WsClose);
            }
            .into_actor(self),
        );
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        info!("WebSocket session stopping for SSE: {}", self.id);
        self.state_addr.do_send(Disconnect { id: self.id.clone() });
        Running::Stop
    }
}

impl Handler<WsText> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsText, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl Handler<WsClose> for WsSession {
    type Result = ();

    fn handle(&mut self, _: WsClose, ctx: &mut Self::Context) -> Self::Result {
        ctx.close(None);
        ctx.stop();
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                debug!("Received WS text for SSE session {}: {}", self.id, text);
                match serde_json::from_str::<JsonRpcRequest>(&text) {
                    Ok(request) => {
                        let session_id = self.id.clone();
                        self.state_addr.clone()
                           .send(IncomingRequest { session_id, request })
                           .into_actor(self)
                           .then(|res, _act, ws_ctx| {
                               match res {
                                   Ok(Some(response)) => {
                                       match serde_json::to_string(&response) {
                                           Ok(resp_text) => ws_ctx.text(resp_text),
                                           Err(e) => error!("Failed to serialize MCP response for WS: {}", e),
                                       }
                                   }
                                   Ok(None) => { }
                                   Err(e) => error!("Mailbox error handling MCP request: {}", e),
                               }
                               fut::ready(())
                           })
                           .wait(ctx);
                    }
                    Err(e) => {
                        warn!("Failed to parse incoming WS message as JsonRpcRequest: {}. Text: {}", e, text);
                         let err_resp = JsonRpcResponse::error(None, JsonRpcError::server_error(ErrorCode::ParseError as i64, e.to_string()));
                         if let Ok(err_str) = serde_json::to_string(&err_resp) {
                             ctx.text(err_str);
                         }
                    }
                }
            }
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
                self.state_addr.do_send(Heartbeat { id: self.id.clone() });
            }
            Ok(ws::Message::Pong(_)) => {
                self.state_addr.do_send(Heartbeat { id: self.id.clone() });
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

pub fn configure_mcp_sse_service(cfg: &mut web::ServiceConfig, state_addr: Addr<McpSseState>) {
    info!("Configuring MCP SSE service endpoint (/api/mcp/sse)...");
    cfg.app_data(Data::new(state_addr))
        .route("/api/mcp/sse", web::get().to(mcp_sse_handler));
} 