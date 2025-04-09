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

# RustyMail Dashboard SSE Testing Plan

## Overview
This document outlines the testing approach for the RustyMail dashboard's Server-Sent Events (SSE) implementation, which provides real-time updates to frontend clients.

## Components Under Test
- SSE endpoint (`/dashboard/api/events`)
- Event broadcasting system
- Client registration and tracking
- Event types and data formats
- Connection handling and heartbeats

## Test Environment Setup
- Mock IMAP server configuration
- Multiple test clients with different lifetimes
- System metrics collection for load testing
- Network condition simulation (latency, disconnections)

## Test Categories

### 1. Connection Management Tests
- Verify client registration process
- Test client disconnection handling
- Verify client tracking in metrics
- Test connection limits and throttling
- Validate client cleanup for inactive connections

### 2. Event Broadcasting Tests
- Verify broadcast to all connected clients
- Test event serialization for different event types
- Verify targeted broadcasts to specific clients
- Test broadcasting with no connected clients
- Validate proper event type and format for each event

### 3. SSE Protocol Compliance Tests
- Verify correct content type (`text/event-stream`)
- Test event format compliance
- Validate proper event ID handling
- Test comment-based heartbeats
- Verify reconnection behavior

### 4. Real-time Events Tests
- Test stats update events (frequency and content)
- Verify client connected/disconnected events
- Test system alert events
- Validate welcome message on connection
- Test custom event broadcasts

### 5. Performance Tests
- Measure broadcast latency under load
- Test with high client connection count (100+)
- Verify memory usage with long-lived connections
- Test reconnection storms (many clients reconnecting simultaneously)
- Measure CPU usage during high-frequency broadcasts

### 6. Error Handling Tests
- Test serialization failures
- Verify dropped client handling
- Test malformed event handling
- Validate rate limiting and backpressure
- Test system recovery after service disruption

## Integration Test Scenarios

### Scenario 1: Dashboard Lifecycle
1. Start RustyMail server
2. Connect multiple dashboard clients
3. Verify stats updates to all clients
4. Disconnect half of clients
5. Verify client disconnected events
6. Connect new clients
7. Verify client connected events

### Scenario 2: System Alerts
1. Connect multiple dashboard clients
2. Trigger system alerts (resource usage, configuration changes)
3. Verify all clients receive alerts with correct formatting
4. Test alert prioritization and ordering

### Scenario 3: Long-running Connection
1. Establish SSE connection
2. Maintain for extended period (1+ hour)
3. Verify heartbeats maintain connection
4. Monitor resource usage
5. Verify all expected events received

## Test Implementation Details

### Test Client Implementation
```rust
async fn test_sse_client() {
    // Connect to SSE endpoint
    let client = reqwest::Client::new();
    let mut resp = client.get("http://localhost:8080/dashboard/api/events")
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Failed to connect to SSE endpoint");

    // Process events
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        let bytes = item.expect("Error reading from stream");
        // Parse and validate SSE events
        // ...
    }
}
```

### Test Framework Integration
- Integration with existing test suite
- Automated test execution in CI pipeline
- Metrics collection and performance baseline comparison

## Test Success Criteria
- All SSE clients receive expected events
- Events properly formatted according to SSE specification
- Client connection/disconnection properly tracked
- System resource usage within acceptable limits under load
- Reconnection works reliably after network interruption

## Implemented Integration Tests

The following integration tests have been implemented in `tests/dashboard_sse_test.rs`:

### 1. Connection and Welcome Message
- `test_sse_connection_receives_welcome`
  - Verifies that a client receives a welcome message upon connection
  - Checks proper event format and client ID assignment

### 2. Stats Updates
- `test_sse_receives_stats_updates`
  - Verifies that clients receive periodic stats updates
  - Validates the content and format of stats events

### 3. Multiple Concurrent Clients
- `test_multiple_concurrent_sse_clients`
  - Tests the system with multiple simultaneous client connections
  - Verifies that all clients receive the expected events
  - Ensures resource usage remains within acceptable limits

### 4. Client Connection Events
- `test_sse_client_connected_events`
  - Verifies that existing clients receive notifications when new clients connect
  - Tests the format and content of client_connected events

### 5. Heartbeat Mechanism
- `test_sse_heartbeat`
  - Validates that the server sends periodic heartbeats to keep connections alive
  - Tests the proper format of heartbeat comments

### 6. System Alerts ✨
- `test_sse_system_alerts`
  - Tests broadcasting of system alerts to connected clients
  - Validates alert format and content
  - Verifies all clients receive the alerts

### 7. Stress Testing ✨
- `test_sse_stress_test`
  - Tests the system under high load with many concurrent clients
  - Measures performance and resource usage
  - Verifies all clients receive expected events
  - Tagged as `#[ignore]` due to resource intensity

### 8. Reconnection Handling ✨
- `test_sse_reconnection`
  - Tests client reconnection behavior
  - Verifies client state management after reconnection
  - Validates that reconnected clients receive welcome events

### Running the Tests

To run all dashboard SSE tests:
```bash
cargo test --test dashboard_sse_test --features integration_tests
```

To run a specific test:
```bash
cargo test --test dashboard_sse_test::test_sse_heartbeat --features integration_tests
```

To run resource-intensive tests:
```bash
cargo test --test dashboard_sse_test --features integration_tests -- --ignored
```
