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
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError, ErrorCode},
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

#[derive(Debug)]
pub struct McpSseState {
    sessions: HashMap<String, SseSession>,
    hb_interval: Duration,
    client_timeout: Duration,
    mcp_handler: Arc<dyn McpHandler>,
    port_state: Arc<TokioMutex<McpPortState>>,
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
        let event = sse::Event::Data(sse::Data::new(&event_data));
        
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
                return Some(JsonRpcResponse::error(None, JsonRpcError::parse_error(e.to_string())));
            }
        };

        let session_id_arc = Arc::new(session_id.to_string());
        let port_state_clone = self.port_state.clone();
        
        match self.mcp_handler.handle_request(session_id_arc, request_json, port_state_clone).await {
            Ok(Some(response_value)) => {
                match serde_json::from_value(response_value) {
                    Ok(resp) => Some(resp),
                    Err(e) => {
                        error!("Failed to deserialize MCP response: {}", e);
                        Some(JsonRpcResponse::error(None, JsonRpcError::internal_error(e.to_string())))
                    }
                }
            }
            Ok(None) => None,
            Err(json_rpc_error) => {
                 Some(JsonRpcResponse::error(None, json_rpc_error))
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
                return Some(JsonRpcResponse::error(None, JsonRpcError::parse_error(e.to_string())));
            }
        };

        let session_id_arc = Arc::new(session_id.to_string());
        
        match mcp_handler.handle_request(session_id_arc, request_json, port_state).await {
            Ok(Some(response_value)) => {
                match serde_json::from_value(response_value) {
                    Ok(resp) => Some(resp),
                    Err(e) => {
                        error!("Failed to deserialize MCP response: {}", e);
                        Some(JsonRpcResponse::error(None, JsonRpcError::internal_error(e.to_string())))
                    }
                }
            }
            Ok(None) => None,
            Err(json_rpc_error) => {
                 Some(JsonRpcResponse::error(None, json_rpc_error))
            }
        }
    }
}

pub async fn mcp_sse_handler(
    req: HttpRequest,
    stream: Payload,
    state_addr: Data<Addr<McpSseState>>,
) -> Result<HttpResponse, ActixError> {
    info!("Handling new MCP SSE connection request");
    
    let session_id = Uuid::new_v4().to_string();
    let state_addr = state_addr.get_ref().clone();

    ws::start_with_addr(state_addr, WsSession { id: session_id }, &req, stream)
}

struct WsSession {
    id: String,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket session started for SSE: {}", self.id);
        let addr = ctx.address();
        let (tx, rx) = mpsc::channel(100);
        
        ctx.state().do_send(Connect { id: self.id.clone(), sender: tx });

        ctx.spawn(async move {
            let mut rx_stream = ReceiverStream::new(rx);
            while let Some(event) = rx_stream.next().await {
                 match serde_json::to_string(&event) {
                     Ok(text) => addr.text(text),
                     Err(e) => error!("Failed to serialize SSE event for WS: {}", e),
                 }
            }
            info!("SSE forwarder task finished for {}", self.id);
            addr.close(None);
        }.into_actor(self));
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        info!("WebSocket session stopping for SSE: {}", self.id);
        ctx.state().do_send(Disconnect { id: self.id.clone() });
        Running::Stop
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
                        ctx.state()
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
                         let err_resp = JsonRpcResponse::error(None, JsonRpcError::parse_error(e.to_string()));
                         if let Ok(err_str) = serde_json::to_string(&err_resp) {
                             ctx.text(err_str);
                         }
                    }
                }
            }
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
                ctx.state().do_send(Heartbeat { id: self.id.clone() });
            }
            Ok(ws::Message::Pong(_)) => {
                ctx.state().do_send(Heartbeat { id: self.id.clone() });
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
    info!("Configuring MCP SSE service endpoint (/mcp/sse)...");
    cfg.app_data(Data::new(state_addr))
        .route("/mcp/sse", web::get().to(mcp_sse_handler));
} 