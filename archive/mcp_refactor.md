# RustyMail Dashboard Integration Plan

## System Architecture Analysis

RustyMail is a Rust-based IMAP client with multiple interfaces:
1. **REST API** - HTTP-based JSON API for IMAP operations
2. **MCP Stdio** - JSON-RPC 2.0 interface over standard I/O
3. **MCP SSE** - JSON-RPC 2.0 interface using Server-Sent Events

The system is designed to:
- Connect to IMAP servers
- Provide a common interface for email operations
- Support multiple transport mechanisms
- Handle authentication and session management

## Integration Strategy

Your dashboard integration will follow these principles:
1. **Dashboard as a Module**: Keep the dashboard UI and API as a self-contained feature
2. **Integration with Main Server**: Make dashboard available when running in REST/MCP* mode
3. **Maintain Separation**: Keep clear boundaries between dashboard and core functionality
4. **Match API Contract**: Implement backend endpoints to match frontend expectations

## Implementation Plan

### 1. Update Configuration System

Add dashboard configuration to `src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub path: Option<String>, // Path to static frontend files
}

// Add to Settings struct
pub struct Settings {
    // Existing fields
    pub dashboard: Option<DashboardConfig>,
}
```

### 2. Implement Dashboard API Routes

Complete the dashboard API implementation in `src/dashboard/api/*.rs` files:

```rust
// src/dashboard/api/routes.rs
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/dashboard")
            .route("/stats", web::get().to(handlers::get_dashboard_stats))
            .route("/clients", web::get().to(handlers::get_connected_clients))
            .route("/config", web::get().to(handlers::get_configuration))
            .route("/chatbot/query", web::post().to(handlers::query_chatbot))
            .route("/events", web::get().to(sse::sse_handler))
    );
}
```

### 3. Implement Dashboard API Models and Handlers

Match the TypeScript types in your frontend with corresponding Rust structs:

```rust
// src/dashboard/api/models.rs
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub active_connections: usize,
    pub request_rate: Vec<RequestRateData>,
    pub system_health: SystemHealth,
    pub last_updated: String,
}

// Additional models to match the frontend types
```

### 4. Integrate Dashboard into Main App

Update `src/main.rs` to include dashboard routes when enabled:

```rust
// Add to HttpServer::new closure
.configure(|cfg| {
    if let Some(dashboard_config) = &settings.dashboard {
        if dashboard_config.enabled {
            // Include dashboard API routes
            cfg.configure(dashboard::api::routes::configure_routes);
            
            // Serve static files if path is provided
            if let Some(path) = &dashboard_config.path {
                cfg.service(
                    actix_files::Files::new("/dashboard", path)
                        .index_file("index.html")
                );
            }
        }
    }
})
```

### 5. Add Static File Serving for Frontend

Add the actix-files dependency to serve the React frontend:

```toml
# Cargo.toml
[dependencies]
actix-files = "0.6"
```

### 6. Create Development Environment Setup

Create a development setup script that:
1. Builds the frontend
2. Copies output to a location accessible to the backend
3. Updates the .env configuration

```bash
#!/bin/bash
# build-dev.sh

# Build frontend
cd frontend/rustymail-app-main
npm run build

# Create dashboard directory if it doesn't exist
mkdir -p ../../dashboard-static

# Copy built files to dashboard-static directory
cp -r dist/* ../../dashboard-static/

# Update .env file to include dashboard configuration
echo "DASHBOARD_ENABLED=true" >> ../../.env
echo "DASHBOARD_PATH=./dashboard-static" >> ../../.env

echo "Dashboard development setup complete"
```

### 7. Implement Dashboard Metrics Collection

Add services implementation to collect real metrics:

```rust
// src/dashboard/services/metrics.rs
impl MetricsService {
    // Real implementation that collects actual system metrics
    pub async fn collect_metrics(&self) {
        let active_connections = // Get from connection pool
        let cpu_usage = // Get system CPU usage
        let memory_usage = // Get system memory usage
        
        // Update metrics store
    }
}
```

### 8. Production Deployment Configuration

For production deployment, include frontend assets in the static directory and update your Dockerfile/deployment scripts. 