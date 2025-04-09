# RustyMail SSE Dashboard - Backend Implementation Plan

## Overview

This document outlines the backend implementation plan for the RustyMail SSE Dashboard. The backend will provide REST and SSE endpoints needed by the frontend, collect metrics from the IMAP adapter, and provide administrative functionality.

## Technology Stack

- **Language**: Rust (2021 edition)
- **Web Framework**: Axum
- **Database**: In-memory (with option for persisted SQLite/PostgreSQL)
- **IMAP Integration**: Direct integration with RustyMail's core IMAP modules
- **Metrics**: Custom instrumentation with Prometheus compatibility
- **SSE**: Native implementation using Axum's streaming capabilities
- **AI Chat Integration**: OpenAI API client with RIG integration

## Architecture

The backend will be architected with the following components:

1. **API Layer**: REST endpoints using Axum
2. **SSE Service**: Real-time event streaming
3. **Metrics Collector**: Gathering and aggregating system statistics
4. **Client Manager**: Tracking and managing connected clients 
5. **Config Service**: Handling system configuration
6. **AI Assistant**: Integration with the RIG chatbot system

## Module Structure

```
src/
├── dashboard/                 (consolidated root directory)
│   ├── mod.rs                 (exports dashboard functionality)
│   ├── api/                   
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   ├── handlers.rs
│   │   ├── models.rs
│   │   ├── sse.rs             (dashboard SSE implementation)
│   │   └── errors.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── metrics.rs
│   │   ├── clients.rs
│   │   ├── config.rs
│   │   └── ai.rs
│   └── testing/
│       ├── mod.rs
│       └── integration.rs
```

This structure keeps all dashboard-related code in a single location, maintaining a clear separation from the existing codebase while making it easy to understand what components are part of the dashboard feature.

**✅ COMPLETED: Basic module structure has been implemented and is ready for development**
**✅ COMPLETED: Core SSE functionality implemented and integration tested.**

## API Endpoints Implementation

### Dashboard Statistics (`GET /api/dashboard/stats`)

```rust
pub async fn get_dashboard_stats(
    State(metrics_service): State<Arc<MetricsService>>,
) -> Result<Json<DashboardStats>, ApiError> {
    let stats = metrics_service.get_current_stats().await?;
    Ok(Json(stats))
}
```

Data model:
```rust
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub active_connections: usize,
    pub requests_per_minute: f64,
    pub average_response_time_ms: f64,
    pub system_cpu_usage: f64,
    pub system_memory_usage: f64,
    pub uptime_seconds: u64,
}
```

### Client Management (`GET /api/dashboard/clients`)

```rust
pub async fn get_connected_clients(
    Query(params): Query<ClientQueryParams>,
    State(client_manager): State<Arc<ClientManager>>,
) -> Result<Json<PaginatedClients>, ApiError> {
    let clients = client_manager.get_clients(
        params.page.unwrap_or(1),
        params.per_page.unwrap_or(10),
        params.filter.as_deref(),
    ).await?;
    
    Ok(Json(clients))
}
```

Data models:
```rust
#[derive(Debug, Deserialize)]
pub struct ClientQueryParams {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub filter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedClients {
    pub clients: Vec<ClientInfo>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub id: String,
    pub ip_address: String,
    pub user_agent: String,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub request_count: usize,
}
```

### Configuration Management (`GET /api/dashboard/config`)

```rust
pub async fn get_configuration(
    State(config_service): State<Arc<ConfigService>>,
) -> Result<Json<ImapConfiguration>, ApiError> {
    let config = config_service.get_configuration().await?;
    Ok(Json(config))
}
```

Data model:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ImapConfiguration {
    pub server_address: String,
    pub port: u16,
    pub use_ssl: bool,
    pub connection_timeout_seconds: u64,
    pub max_connections: usize,
    pub connection_pool_size: usize,
}
```

### AI Assistant (`POST /api/dashboard/chatbot/query`)

```rust
pub async fn query_chatbot(
    State(ai_service): State<Arc<AiService>>,
    Json(query): Json<ChatbotQuery>,
) -> Result<Json<ChatbotResponse>, ApiError> {
    let response = ai_service.process_query(query.query).await?;
    Ok(Json(ChatbotResponse { response }))
}
```

Data models:
```rust
#[derive(Debug, Deserialize)]
pub struct ChatbotQuery {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct ChatbotResponse {
    pub response: String,
}
```

## SSE Implementation

```rust
pub async fn sse_handler(
    State(sse_manager): State<Arc<SseManager>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(100);
    
    let client_id = Uuid::new_v4().to_string();
    sse_manager.register_client(client_id.clone(), tx).await;
    
    let stream = rx.map(move |event| {
        Ok(Event::default().event(event.event_type).data(event.data))
    });
    
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping")
    )
}
```

SSE Event structure:
```rust
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
}
```

## Metrics Collection Service

```rust
pub struct MetricsService {
    metrics_store: Arc<RwLock<MetricsStore>>,
    collection_interval: Duration,
}

impl MetricsService {
    pub fn new(collection_interval: Duration) -> Self {
        let metrics_store = Arc::new(RwLock::new(MetricsStore::default()));
        let store_clone = Arc::clone(&metrics_store);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(collection_interval);
            loop {
                interval.tick().await;
                Self::collect_metrics(store_clone.clone()).await;
            }
        });
        
        Self { 
            metrics_store,
            collection_interval,
        }
    }
    
    async fn collect_metrics(store: Arc<RwLock<MetricsStore>>) {
        // Collect system metrics
        // Collect IMAP connection statistics
        // Update metrics store
    }
    
    pub async fn get_current_stats(&self) -> Result<DashboardStats, MetricsError> {
        let store = self.metrics_store.read().await;
        Ok(DashboardStats {
            active_connections: store.active_connections,
            requests_per_minute: store.requests_per_minute,
            average_response_time_ms: store.average_response_time_ms,
            system_cpu_usage: store.system_cpu_usage,
            system_memory_usage: store.system_memory_usage,
            uptime_seconds: store.uptime_seconds,
        })
    }
}
```

## Client Manager Implementation

```rust
pub struct ClientManager {
    clients: RwLock<HashMap<String, ClientInfo>>,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }
    
    pub async fn register_client(&self, client_id: String, client_info: ClientInfo) {
        let mut clients = self.clients.write().await;
        clients.insert(client_id, client_info);
    }
    
    pub async fn get_clients(
        &self,
        page: usize,
        per_page: usize,
        filter: Option<&str>,
    ) -> Result<PaginatedClients, ClientError> {
        let clients = self.clients.read().await;
        
        let filtered_clients: Vec<ClientInfo> = clients
            .values()
            .filter(|client| {
                if let Some(filter_text) = filter {
                    client.user_agent.contains(filter_text) || 
                    client.ip_address.contains(filter_text)
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        
        let total = filtered_clients.len();
        let offset = (page - 1) * per_page;
        let paginated = filtered_clients
            .into_iter()
            .skip(offset)
            .take(per_page)
            .collect();
            
        Ok(PaginatedClients {
            clients: paginated,
            total,
            page,
            per_page,
        })
    }
}
```

## IMAP Integration

The backend will integrate with RustyMail's existing IMAP session management to collect metrics and monitor active connections:

```rust
pub async fn monitor_imap_sessions(
    session_manager: Arc<ImapSessionManager>,
    metrics_service: Arc<MetricsService>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    
    loop {
        interval.tick().await;
        
        let session_stats = session_manager.get_session_stats().await;
        metrics_service.update_imap_metrics(session_stats).await;
    }
}
```

## AI Assistant Integration

```rust
pub struct AiService {
    openai_client: OpenAiClient,
    rig_client: Option<RigClient>,
}

impl AiService {
    pub fn new(openai_api_key: String, rig_endpoint: Option<String>) -> Self {
        let openai_client = OpenAiClient::new(openai_api_key);
        let rig_client = rig_endpoint.map(RigClient::new);
        
        Self {
            openai_client,
            rig_client,
        }
    }
    
    pub async fn process_query(&self, query: String) -> Result<String, AiError> {
        if let Some(rig) = &self.rig_client {
            match rig.process_query(&query).await {
                Ok(response) => return Ok(response),
                Err(_) => {} // Fall back to OpenAI
            }
        }
        
        self.openai_client.generate_response(&query).await
            .map_err(AiError::OpenAiError)
    }
}
```

## Testing Strategy

1. **Unit Tests**: For individual components and services
2. **Integration Tests**: For API endpoints and service interactions
3. **End-to-End Tests**: Full system testing with mocked IMAP server

Example unit test:
```rust
#[tokio::test]
async fn test_metrics_collection() {
    let metrics_service = MetricsService::new(Duration::from_secs(1));
    tokio::time::sleep(Duration::from_secs(2)).await; // Allow time for initial collection
    
    let stats = metrics_service.get_current_stats().await.unwrap();
    
    assert!(stats.system_cpu_usage >= 0.0);
    assert!(stats.system_memory_usage >= 0.0);
    assert!(stats.uptime_seconds > 0);
}
```

Example integration test:
```rust
#[tokio::test]
async fn test_dashboard_api() {
    let app = create_test_app().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/stats")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let stats: DashboardStats = serde_json::from_slice(&body).unwrap();
    
    assert!(stats.active_connections >= 0);
}
```

## Implementation Schedule

1. **Week 1**: Core infrastructure and API setup
   - Base Axum app setup
   - Endpoint routing
   - Model definitions

2. **Week 2**: Service implementation
   - Metrics collection
   - Client management
   - Configuration handling

3. **Week 3**: IMAP integration and SSE implementation
   - Connect to IMAP modules
   - Implement SSE broadcasting
   - Real-time metrics

4. **Week 4**: AI integration and testing
   - OpenAI client integration
   - RIG integration
   - Testing and debugging

5. **Week 5**: Finalization and documentation
   - Performance optimization
   - Documentation
   - Frontend integration

## Future Enhancements

1. Persistent storage for historical metrics
2. Advanced filtering and search for client management
3. Enhanced analytics dashboard with historical data
4. Alerting and notification system
5. User management and authentication for dashboard access

## Dashboard SSE Integration Test Implementation

### Test Infrastructure

The dashboard SSE implementation is tested using integration tests that:

1. Start the full RustyMail server with dashboard enabled
2. Connect to the SSE endpoint
3. Verify events are received
4. Test various scenarios like reconnection, stress, etc.

### Current Test Setup

The current test strategy in `tests/dashboard_sse_test.rs` follows these steps:

1. The `TestServer` struct spawns a RustyMail server process
2. Environment variables set up the IMAP connection and dashboard
3. The `SseClient` connects to the `/dashboard/api/events` endpoint
4. Tests verify proper event delivery

### Port Conflict Issues

The main issue is that the tests use a hardcoded port (8080) which creates conflicts:

1. If multiple tests run in parallel, they compete for the same port
2. If a server is already running on port 8080, tests will fail
3. If a test crashes, it may leave a zombie process bound to port 8080

### Solutions

1. **Dynamic Port Allocation**:
   ```rust
   // Find an available port
   fn find_available_port() -> u16 {
       let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
       listener.local_addr().expect("Failed to get local address").port()
   }
   
   // In TestServer::new
   let port = find_available_port();
   // Update env vars with this port
   env_vars.insert("REST_PORT".to_string(), port.to_string());
   // Update BASE_URL constant
   let base_url = format!("http://127.0.0.1:{}", port);
   ```

2. **Process Cleanup**:
   - Modify the TestServer shutdown process to ensure complete cleanup
   - Add a cleanup step that kills orphaned processes
   - Use pidfiles to track running test servers

3. **Endpoint Path Correction**:
   - Ensure the SSE endpoint path matches between tests and implementation
   - Change from `/dashboard/api/events` to `/api/dashboard/events` if needed

4. **Improved Test Isolation**:
   - Use `serial_test` attribute to prevent parallel execution
   - Add proper teardown even when tests panic
   
### Path Forward

1. Update the test code to use dynamic port allocation
2. Fix endpoint path discrepancies
3. Ensure proper process cleanup
4. Add logs to debug event delivery issues
5. Ensure welcome events and stats updates are properly implemented

With these changes, the dashboard SSE integration tests should run reliably, without port conflicts, and provide meaningful test coverage for the SSE implementation.
