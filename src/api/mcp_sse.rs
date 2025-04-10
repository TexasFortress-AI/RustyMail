use actix_web::{
    web::{self, Bytes, Data, Path, Responder},
    Error as ActixError, HttpRequest, HttpResponse,
};
use actix_web_lab::sse::{self, Sse, Event};
use futures_util::stream::{Stream, StreamExt};
use tokio::{
    sync::{
        Mutex as TokioMutex,
        RwLock,
        mpsc,
        broadcast::{self, Receiver}
    },
    time::{Duration, Instant},
};
use std::{
    sync::Arc,
    collections::HashMap,
    convert::Infallible,
};
use uuid::Uuid;
use log::{debug, error, info, warn};
use serde::Serialize;
use serde_json::{self, json, Value};

// Local imports
use crate::{
    api::rest::AppState,
    imap::ImapSessionFactory,
    mcp::{
        handler::McpHandler,
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError},
        error_codes::ErrorCode,
    },
};

// Re-exports for use in other modules
pub use actix_web::{
    web::{App, HttpServer},
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

pub struct McpSseState {
    clients: Arc<RwLock<HashMap<Uuid, mpsc::Sender<sse::Event>>>>,
    mcp_handler: Arc<dyn McpHandler>,
}

impl McpSseState {
    pub fn new(mcp_handler: Arc<dyn McpHandler>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            mcp_handler,
        }
    }

    async fn add_client(&self, client_id: Uuid) -> Result<mpsc::Receiver<sse::Event>, JsonRpcError> {
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
}

async fn sse_connection_handler(
    app_state: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ActixError> {
    let client_id = path.into_inner();
    let sse_state = app_state.sse_state();

    info!("MCP SSE: Handling new connection for client_id: {}", client_id);

    match sse_state.add_client(client_id).await {
        Ok(rx) => {
            info!("MCP SSE: Client {} successfully added, returning stream.", client_id);
            
            let sse_response = sse::Sse::from_infallible_receiver(rx)
                .with_keep_alive(Duration::from_secs(15));
            
            sse_state.start_heartbeat(client_id);

            Ok(sse_response.respond_to(&req)?)
        },
        Err(e) => {
            error!("MCP SSE: Failed to add client {}: {:?}", client_id, e);
            Ok(HttpResponse::InternalServerError().json(e))
        }
    }
}

pub fn config_sse(cfg: &mut web::ServiceConfig, sse_state: McpSseState) {
    cfg.app_data(Data::new(sse_state))
        .route("/events", web::get().to(sse_connection_handler));
}

pub fn configure_sse_service(cfg: &mut web::ServiceConfig) {
    info!("Configuring MCP SSE service endpoint (/mcp/events/{client_id})...");
    cfg.service(
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

pub struct McpSseServer {
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
                .app_data(Data::new(event_tx.clone()))
                .route("/events", web::get().to(handle_sse))
        })
        .bind("127.0.0.1:8080")?
        .run()
        .await
    }
}

async fn handle_sse(
    port_state: Data<Arc<McpPortState>>,
    event_tx: Data<broadcast::Sender<Value>>,
) -> Result<HttpResponse, Error> {
    let mut rx = event_tx.subscribe();
    
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .streaming(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield Ok::<_, Error>(web::Bytes::from(format!("data: {}\n\n", event)));
            }
        }))
}

pub async fn spawn_mcp_sse_server(port_state: Arc<McpPortState>) -> std::io::Result<()> {
    let server = McpSseServer::new(port_state);
    server.run().await
} 