// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use rustymail::config::Settings;
// Remove direct ImapClient import if only used for connect check, keep if factory needs it explicitly
// use rustymail::imap::ImapClient;
use rustymail::api::rest::{AppState, configure_rest_service};
use rustymail::api::auth::ApiKeyStore;
use rustymail::api::rate_limit::{RateLimitConfig, RateLimitMiddleware};
use std::sync::Arc;
use dotenvy::dotenv;
use log::{info, error, warn};
// Remove tool registry creation
// use rustymail::mcp_port::create_mcp_tool_registry;
// --- Add McpHandler and SdkMcpAdapter imports ---
use rustymail::mcp::handler::McpHandler;
use rustymail::mcp::adapters::sdk::SdkMcpAdapter;
// --- End imports ---
use env_logger;
use rustymail::dashboard;
use rustymail::dashboard::api::SseManager;
use rustymail::api::openapi_docs;
// --- Add imports for factory ---
use rustymail::imap::client::ImapClient; // Needed for the factory closure
// --- End imports for factory ---
// Remove non-existent imports
// use rustymail::mcp::adapters::stdio::run_stdio_handler;
// use rustymail::mcp::handler::JsonRpcHandler;
use rustymail::prelude::*; // Import many common types

// Use jemalloc as the global allocator for better memory management
// jemalloc releases memory back to the OS, unlike the default system allocator
// This prevents memory bloat from IMAP session BytePools being held by the allocator
// NOTE: Can be disabled with --features system-alloc or --features mimalloc-alloc for testing
#[cfg(all(not(target_env = "msvc"), not(feature = "dhat-heap"), not(feature = "system-alloc"), not(feature = "mimalloc-alloc")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

// Use mimalloc allocator (known for better memory return to OS on macOS)
#[cfg(feature = "mimalloc-alloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Enable DHAT heap profiler when compiled with --features dhat-heap
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize DHAT profiler if enabled
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Load .env file if present
    dotenv().ok();

    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    info!("Starting RustyMail server...");

    // Log allocator info
    #[cfg(all(not(target_env = "msvc"), not(feature = "dhat-heap")))]
    info!("Using jemalloc allocator for better memory management");

    // Load configuration
    info!("Loading configuration...");
    let settings = match Settings::new(None) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to load application settings: {:?}", e);
            panic!("Configuration loading failed: {:?}", e);
        }
    };

    // Determine active interface from settings and print config details
    let active_interface = settings.interface.clone();
    info!("Using interface: {:?}", active_interface);
    info!("IMAP config: host={}, port={}, user={}", settings.imap_host, settings.imap_port, settings.imap_user);

    // --- Perform initial IMAP connection check --- (Optional but good for validation)
    // TEMPORARILY DISABLED: Skip IMAP connection check for dashboard testing
    info!("Skipping initial IMAP connection check for dashboard testing...");
    /*
    match ImapClient::<AsyncImapSessionWrapper>::connect(
        &settings.imap_host,
        settings.imap_port,
        &settings.imap_user,
        &settings.imap_pass,
    ).await {
        Ok(client) => {
            info!("Initial IMAP connection successful. Logging out...");
            // Use try_logout to avoid panicking if logout fails
            if let Err(logout_err) = client.logout().await {
                 warn!("Failed to logout after initial connection check: {:?}", logout_err);
            }
        }
        Err(e) => {
            error!("Initial IMAP connection failed: {:?}. Server startup aborted.", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("IMAP connection failed: {:?}", e)));
        }
    }
    */

    // --- Create IMAP Session Factory ---
    use futures_util::future::BoxFuture;
    let imap_settings = settings.clone(); // Clone settings needed for the factory
    let raw_imap_session_factory: Box<dyn Fn() -> BoxFuture<'static, Result<ImapClient<AsyncImapSessionWrapper>, ImapError>> + Send + Sync> = Box::new(move || {
        let settings_clone = imap_settings.clone(); // Clone again for the async block
        Box::pin(async move {
            info!("ImapSessionFactory: Creating new IMAP session..."); // Add log
            let client = ImapClient::<AsyncImapSessionWrapper>::connect(
                &settings_clone.imap_host,
                settings_clone.imap_port,
                &settings_clone.imap_user,
                &settings_clone.imap_pass,
            ).await.map_err(|e| {
                error!("ImapSessionFactory: Failed to connect: {:?}", e); // Add error log
                e // Return the original ImapError
            })?;
            info!("ImapSessionFactory: New IMAP session created successfully."); // Add success log
            Ok(client) // <-- Return the client directly
        })
    });
    // Wrap the factory in a cloneable wrapper
    let imap_session_factory = CloneableImapSessionFactory::new(raw_imap_session_factory);
    info!("IMAP Session Factory created.");

    // --- Create Connection Pool ---
    use rustymail::connection_pool::{ConnectionPool, ConnectionFactory, PoolConfig};
    use std::time::Duration;

    // Create connection factory that uses our IMAP session factory
    struct ImapConnectionFactory {
        session_factory: CloneableImapSessionFactory,
    }

    #[async_trait::async_trait]
    impl ConnectionFactory for ImapConnectionFactory {
        async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError> {
            let client = self.session_factory.create_session().await?;
            Ok(Arc::new(client))
        }

        async fn validate(&self, client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool {
            // Send NOOP command to verify connection is alive
            // This serves as both a health check and keepalive
            match client.noop().await {
                Ok(_) => {
                    log::debug!("Connection validated successfully via NOOP");
                    true
                }
                Err(e) => {
                    log::warn!("Connection validation failed via NOOP: {}", e);
                    false
                }
            }
        }
    }

    let pool_config = PoolConfig::default();

    let connection_factory = Arc::new(ImapConnectionFactory {
        session_factory: imap_session_factory.clone(),
    });

    let connection_pool = ConnectionPool::new(connection_factory, pool_config.clone());
    info!("Connection Pool created with min={}, max={} connections", pool_config.min_connections, pool_config.max_connections);

    // --- Create Tool Registry (REMOVED) ---
    // let tool_registry_rest = create_mcp_tool_registry(imap_client_rest.clone());
    // info!("MCP Tool Registry created.");

    // --- Create MCP Handler ---
    // TODO: Implement SdkMcpAdapter::new properly
    let mcp_handler: Arc<dyn McpHandler> = Arc::new(
        SdkMcpAdapter::new(imap_session_factory.clone())
            .expect("SdkMcpAdapter initialization failed")
    );
    info!("MCP Handler (SdkMcpAdapter) created.");

    // --- Create Shared State (SSE not implemented yet) ---
    // let sse_state = Arc::new(TokioMutex::new(SseState::new()));
    // info!("SSE shared state initialized.");

    // --- Create AppState manually (no new method) ---
    let session_manager = Arc::new(SessionManager::new(Arc::new(settings.clone())));
    let api_key_store = Arc::new(ApiKeyStore::new());
    api_key_store.init_from_env().await;
    let app_state = AppState {
        settings: Arc::new(settings.clone()),
        mcp_handler: mcp_handler.clone(),
        session_manager: session_manager.clone(),
        dashboard_state: None, // Will be set later
        api_key_store: api_key_store.clone(),
    };
    info!("Application state initialized.");

    // --- Dashboard Setup (remains largely the same, but might need factory later) ---
    let config = web::Data::new(settings.clone());
    info!("Dashboard configuration initialized.");

    // Dashboard services initialization with connection pool
    let dashboard_state = dashboard::services::init(
        config.clone(),
        imap_session_factory.clone(),
        connection_pool
    ).await;
    info!("Dashboard state initialized.");

    // Start background metrics collection task (pass only connection pool to avoid circular reference)
    dashboard_state.metrics_service.start_background_collection(Arc::clone(&dashboard_state.connection_pool));

    // Start sync process spawner instead of in-process sync
    // This runs sync in a separate process that exits after each cycle,
    // ensuring memory is fully reclaimed by the OS
    start_sync_process_spawner();
    info!("Sync process spawner started");

    // Start outbox worker for asynchronous email sending
    let outbox_worker = Arc::new(rustymail::dashboard::services::OutboxWorker::new(
        Arc::clone(&dashboard_state.outbox_queue_service),
        Arc::clone(&dashboard_state.smtp_service),
        imap_session_factory.clone(),
        Arc::clone(&dashboard_state.account_service),
        Arc::clone(&dashboard_state.cache_service),
    ));
    tokio::spawn(async move {
        outbox_worker.start().await;
    });
    info!("Outbox worker started");

    // Start health monitoring service
    if let Some(ref health_service) = dashboard_state.health_service {
        Arc::clone(health_service).start_monitoring().await;
        info!("Health monitoring service started");
    }

    // Start event publishers for dashboard integration
    dashboard::services::event_integration::start_event_publishers(Arc::new(dashboard_state.as_ref().clone())).await;
    info!("Event publishers started");

    // Start MCP session cleanup task
    rustymail::api::mcp_http::start_session_cleanup();
    info!("MCP session cleanup task started");

    // Create and initialize SSE manager for dashboard
    let sse_manager = Arc::new(SseManager::new(
        Arc::clone(&dashboard_state.metrics_service),
        Arc::clone(&dashboard_state.client_manager)
    ));
    info!("Dashboard SSE Manager initialized.");
    // --- End Dashboard Setup ---

    // --- Start HTTP Server ---
    let rest_config = settings.rest.as_ref().cloned()
        .expect("REST configuration is required - ensure REST_HOST and REST_PORT environment variables are set");
    let (host, port) = (rest_config.host.clone(), rest_config.port);
    let listen_addr = format!("{}:{}", host, port);
    info!("Starting HTTP server (REST & MCP SSE) on {}", listen_addr);

    // Clone state needed for the broadcast task
    let sse_manager_clone_for_task = Arc::clone(&sse_manager);
    let dashboard_state_clone_for_task = dashboard_state.clone();

    let server = HttpServer::new(move || {
        // Configure rate limiting from environment variables
        let rate_limit_config = RateLimitConfig::from_env();
        info!("Rate limiting configured: {} req/min, {} req/hour per IP",
            rate_limit_config.per_ip_per_minute,
            rate_limit_config.per_ip_per_hour);

        // Configure CORS with secure whitelist-based approach
        let allowed_origins_str = std::env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| {
                warn!("ALLOWED_ORIGINS not set, defaulting to localhost:9439 only for development safety");
                "http://localhost:9439,http://127.0.0.1:9439".to_string()
            });

        let allowed_origins: Vec<String> = allowed_origins_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if allowed_origins.is_empty() {
            warn!("No valid ALLOWED_ORIGINS configured, CORS will reject all cross-origin requests");
        }

        // Build CORS configuration with specific allowed origins
        let mut cors = Cors::default()
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::ORIGIN,
            ])
            .allowed_header("X-Api-Key")
            .supports_credentials()
            .max_age(3600);

        // Add each allowed origin
        for origin in &allowed_origins {
            cors = cors.allowed_origin(origin);
        }

        let mut app = App::new()
            // --- Register updated state ---
            .app_data(web::Data::new(app_state.clone()))        // Core AppState (handler, factory)
            .app_data(web::Data::new(imap_session_factory.clone())) // Pass factory directly for REST handlers
            // .app_data(web::Data::new(sse_state.clone()))     // SSE not implemented yet
            // --- End updated state ---
            .app_data(config.clone())                             // Dashboard config
            .app_data(dashboard_state.clone())                  // Dashboard state
            .app_data(web::Data::new(sse_manager.clone()))      // Dashboard SSE Manager
            .wrap(RateLimitMiddleware::new(rate_limit_config.clone()))
            .wrap(cors)
            .wrap(actix_web::middleware::Logger::default())
            .wrap(dashboard::api::middleware::Metrics)
            // Configure routes
            .configure(configure_rest_service)                // RustyMail REST API
            .configure(openapi_docs::configure_openapi)       // OpenAPI/Swagger documentation
            // .configure(configure_sse_service)              // SSE not implemented yet
            .configure(|cfg| dashboard::api::init_routes(cfg)) // Dashboard API routes
            .configure(rustymail::api::mcp_http::configure_mcp_routes); // MCP Streamable HTTP transport

        // Dashboard static files are served by Vite dev server (port 9439) in development
        // For production, use a reverse proxy (nginx) or CDN to serve static files
        // The REST API server (this server) only handles API requests on port 9437
        app // Return the configured app
    })
    .bind(&listen_addr)
    .map_err(|e| {
        error!("Failed to bind server to {}: {}", listen_addr, e);
        e
    })?
    .workers(1)  // TEMPORARY: Use single worker to debug memory leak
    .run();

    // Spawn the Dashboard SSE broadcast task
    info!("Spawning Dashboard SSE broadcast task...");
    tokio::spawn(async move {
        sse_manager_clone_for_task.start_stats_broadcast(dashboard_state_clone_for_task).await;
    });

    // Await the server
    info!("Server run loop starting.");
    server.await
}

/// Start a background task that spawns the sync process periodically.
/// The sync process runs in a separate process that exits after each sync cycle,
/// ensuring all memory allocated during sync is returned to the OS.
fn start_sync_process_spawner() {
    use std::time::Duration;

    let sync_interval: u64 = std::env::var("SYNC_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300); // Default: 5 minutes

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(sync_interval));
        interval.tick().await; // Skip first immediate tick

        loop {
            interval.tick().await;

            // Find the sync binary - check multiple locations
            let sync_binary = if std::path::Path::new("./target/release/rustymail-sync").exists() {
                "./target/release/rustymail-sync"
            } else if std::path::Path::new("./rustymail-sync").exists() {
                "./rustymail-sync"
            } else {
                // Try to find it in PATH
                "rustymail-sync"
            };

            match std::process::Command::new(sync_binary).spawn() {
                Ok(child) => {
                    info!("Spawned sync process (pid: {:?})", child.id());
                }
                Err(e) => {
                    error!("Failed to spawn sync process '{}': {}", sync_binary, e);
                }
            }
        }
    });
}
