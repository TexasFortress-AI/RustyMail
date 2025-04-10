use actix_web::{
    rt,
    web::{self, Bytes, Data, Path},
    Error as ActixError, HttpRequest, HttpResponse, Responder,
    get,
};
use actix_web_lab::sse::{self, Sse, Event};
use futures_util::stream::Stream;
use tokio::time::{Duration};
use tokio::sync::{
    Mutex as TokioMutex,
    RwLock
};
use std::sync::{Arc};
use std::collections::HashMap;
use std::convert::Infallible;
use uuid::Uuid;
use crate::{
    mcp::{
        handler::McpHandler,
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError},
    },
};
use log::{debug, error, info, warn};
use serde::Serialize;
use serde_json::{self};
use tokio::time::interval;
use tokio::sync::mpsc;
use crate::api::rest::ApiError;

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
    port_state: Arc<tokio::sync::Mutex<McpPortState>>,
}

impl SseState {
    pub fn new(mcp_handler: Arc<dyn McpHandler>, port_state: Arc<tokio::sync::Mutex<McpPortState>>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            mcp_handler,
            port_state,
        }
    }

    async fn add_client(&self, client_id: Uuid) -> Result<mpsc::Receiver<sse::Event>, ApiError> {
        let (tx, rx) = mpsc::channel(32);
        self.clients.write().await.insert(client_id, tx);
        Ok(rx)
    }

    async fn remove_client(&self, client_id: &Uuid) {
        self.clients.write().await.remove(client_id);
    }

    pub async fn broadcast_event(&self, event: sse::Event) {
        let clients = self.clients.read().await;
        for (_, tx) in clients.iter() {
            let _ = tx.send(event.clone()).await;
        }
    }

    async fn handle_mcp_request(&self, client_id: &Uuid, request: JsonRpcRequest) {
        let request_json = match serde_json::to_value(request.clone()) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to serialize MCP request to JSON: {}", e);
                self.broadcast_event(Event::Data(sse::Data::new("mcp_error"))).await;
                return;
            }
        };
        
        let response_json = self.mcp_handler.handle_request(self.port_state.clone(), request_json).await;
        
        let response: JsonRpcResponse = match serde_json::from_value(response_json.clone()) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse MCP response from JSON: {}", e);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError::internal_error("Failed to parse response")),
                }
            }
        };
        
        if let Ok(response_str) = serde_json::to_string(&response) {
            self.broadcast_event(Event::Data(sse::Data::new(response_str))).await;
        } else {
            error!("Failed to serialize MCP response for client {}", client_id);
            let error_response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError::internal_error("Failed to serialize response")),
            };
            if let Ok(err_json) = serde_json::to_string(&error_response) {
                self.broadcast_event(Event::Data(sse::Data::new(err_json))).await;
            }
        }
    }
}

pub async fn sse_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: Data<SseState>,
) -> impl Responder {
    let client_id = Uuid::new_v4();
    info!("New SSE client connected: {}", client_id);

    let rx = match state.add_client(client_id).await {
        Ok(rx) => rx,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to add SSE client: {}", e));
        }
    };

    let stream = rx.map(|event| Ok::<_, actix_web::Error>(event));

    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .streaming(stream)
}

async fn broadcast_task(sse_state: Arc<TokioMutex<SseState>>) {
    let mut interval = interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let state = sse_state.lock().await;
        let event = Event::Data(sse::Data::new("tick"));
        state.broadcast_event(event).await;
    }
}

pub fn configure_sse_service(cfg: &mut web::ServiceConfig) {
    info!("Configuring SSE service endpoint (/events)...");
    cfg.service(
        web::resource("/events")
            .route(web::get().to(sse_handler))
    );
    info!("SSE service configured.");
} 