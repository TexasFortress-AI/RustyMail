---
description: 
globs: 
alwaysApply: true
---
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
**✅ COMPLETED: Basic REST API endpoints and services implemented with initial tests.**
**✅ COMPLETED: Metrics middleware implemented for request timing.**
**✅ COMPLETED: Service initialization integrated into main application.**
**✅ COMPLETED: SSE integration testing refined (dynamic ports, improved setup).**
**✅ COMPLETED: All core dashboard API endpoints finalized (Stats, Clients, Config, Chatbot(mock)).**

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
3. **End-to-End Tests**: Full system testing with mocked IMAP server (Note: Live IMAP server used in current SSE tests)

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

(Revised based on current progress - Approximate)

1. **Weeks 1-2**: Core infrastructure and API setup
   - **✅ COMPLETED**

2. **Weeks 2-3**: Service implementation & SSE
   - **✅ COMPLETED**

3. **Weeks 3-4**: IMAP integration and SSE Testing Refinement
   - Connect to IMAP modules for metrics (**✅ COMPLETED - Using SSE count proxy**)
   - Implement SSE broadcasting for stats (**✅ COMPLETED**)
   - Refine SSE integration tests (dynamic ports, etc.) (**✅ COMPLETED**)

4. **Week 4-5**: API Completion & Testing
   - Finalize Client Management API endpoint & Service Logic (**✅ COMPLETED**)
   - Finalize Config API endpoint & Service Logic (**✅ COMPLETED**)
   - Finalize Chatbot API endpoint & Service Logic (**✅ COMPLETED - Mock AI Used**)
   - Basic testing for API endpoints (**✅ COMPLETED**)

5. **Week 5+**: Reliability, Feature Completion & Finalization
   - Fix `ClientManager` cleanup bug (**NEXT - Bug fix**)
   - Ensure proper test process cleanup (`TestServer` drop logic in `tests/`)
   - Implement real AI Service logic (OpenAI) (**✅ COMPLETED**)
   - Refactor AI provider using ports/adapter pattern (OpenAI, OpenRouter) (**✅ COMPLETED**)
   - Add comprehensive integration and edge-case tests
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

1. Update the test code to use dynamic port allocation **✅ COMPLETED**
2. Fix endpoint path discrepancies **✅ COMPLETED (Implicitly handled by current setup)**
3. Fix `ClientManager` background cleanup task **✅ COMPLETED**
4. Ensure proper test process cleanup (`TestServer` Drop) **✅ COMPLETED**
5. Implement real AI Service logic (OpenAI) **✅ COMPLETED**
6. Implement IMAP metrics collection (**✅ COMPLETED - Reporting SSE client count as proxy**)
7. Finalize API endpoints for Clients, Config, and AI (**✅ COMPLETED - AI uses mock logic initially, now uses OpenAI**)
8. **NEXT:** Add comprehensive integration and edge-case tests
9. Code cleanup and documentation (**TODO**)

All core backend implementation tasks outlined in this plan are complete.
See the "Future Considerations" section for potential next steps outside the original scope.

## Implementation Checklist

**Phase 1: Core Infrastructure & Setup**
- [x] Setup base Axum application
- [x] Define initial API endpoint routing (`src/dashboard/api/routes.rs`)
- [x] Define initial data models (`src/dashboard/api/models.rs`)
- [x] Establish basic module structure (`src/dashboard/...`)
- [x] Integrate dashboard module into `main.rs`

**Phase 2: Service Implementation & SSE**
- [x] Implement `MetricsService` foundation (system metrics, request tracking) (`src/dashboard/services/metrics.rs`)
- [x] Implement `ClientManager` foundation (`src/dashboard/services/clients.rs`)
- [x] Implement `ConfigService` foundation (`src/dashboard/services/config.rs`)
- [x] Implement `AiService` foundation (with mock logic) (`src/dashboard/services/ai.rs`)
- [x] Implement `SseManager` and SSE broadcasting (`src/dashboard/api/sse.rs`)
- [x] Integrate service initialization in `src/dashboard/services/mod.rs`
- [x] Implement Metrics middleware (`src/dashboard/api/middleware.rs`)

**Phase 3: API Endpoint Finalization & SSE Refinement**
- [x] Finalize `GET /api/dashboard/stats` endpoint and handler
- [x] Finalize `GET /api/dashboard/clients` endpoint and handler
- [x] Finalize `GET /api/dashboard/config` endpoint and handler
- [x] Finalize `POST /api/dashboard/chatbot/query` endpoint and handler (using mock AI initially)
- [x] Implement SSE broadcasting for stats updates
- [x] Refine SSE integration tests (dynamic ports, basic scenarios) (`tests/dashboard_sse_test.rs`)
- [x] Refine metrics reporting (`active_sse_connections`)

**Phase 4: Reliability & Feature Completion**
- [x] Fix `ClientManager` background cleanup task bug (`src/dashboard/services/clients.rs`)
- [x] Ensure proper test process cleanup (`TestServer` drop logic in `tests/`)
- [x] Implement real AI Service logic (OpenAI) (`src/dashboard/services/ai.rs`)
- [x] Refactor AI provider using ports/adapter pattern (OpenAI, OpenRouter) (`src/dashboard/services/ai/providers/`)

**Phase 7: Use OFFICIAL MCP Lib**
- [ ] Refactor our https://github.com/modelcontextprotocol/rust-sdk for use in our frontend mcp client and our backend mcp stdio and mcp sse servers. Again, Ports/Adapters. The current MCP code should be moved to RustyNailMCP mcp lib adapter, and we use the new rust-sdk as the new default adapter.
This allows us to create unit tests in the future that can be used to test both adapters, since they share the exact same api.

*Core backend implementation complete per this plan.* 

## Future Considerations

The following items were part of the original Phase 5 finalization but are deferred for potential future work:

- **Comprehensive Testing**: Add more detailed integration tests covering various success scenarios, edge cases, and particularly error handling for all API endpoints and SSE events.
- **Performance Optimization**: Analyze performance under load and optimize critical paths if necessary.
- **Code Cleanup & Documentation**: Perform a thorough code review focusing on clarity, consistency, and adding comprehensive doc comments (especially for public APIs and complex logic).
- **Frontend Integration**: Review the backend API in the context of frontend needs, potentially adjusting models or endpoints for better integration.
- **Real IMAP Metrics**: Update `MetricsService` to gather actual IMAP connection stats. (Field renamed, but data source is currently SSE client count proxy. Requires identifying/implementing access to true IMAP connection count).
- **Advanced Features**: Consider items from the "Future Enhancements" section like persistent metrics, advanced client filtering, alerting, etc.
