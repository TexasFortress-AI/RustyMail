use actix_web::{web, App, HttpServer};
use rustymail::config::Settings;
use rustymail::imap::ImapClient;
use rustymail::api::rest::{AppState, configure_rest_service};
use std::sync::Arc;
use dotenvy::dotenv;
use log::{info, error, warn};
use rustymail::mcp_port::create_mcp_tool_registry;
use rustymail::api::mcp_sse::SseAdapter;
use rustymail::api::mcp_sse::SseState;
use tokio::sync::Mutex as TokioMutex;
use env_logger;
use rustymail::dashboard;
use rustymail::dashboard::api::SseManager;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present
    dotenv().ok();

    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    info!("Starting RustyMail server...");

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

    // --- Create Core IMAP Client --- (assuming rest/sse mode)
    info!("Connecting to IMAP server (for REST/SSE mode)...");
    let imap_client_rest = Arc::new(ImapClient::connect(
        &settings.imap_host,
        settings.imap_port,
        &settings.imap_user,
        &settings.imap_pass,
    ).await.map_err(|e| {
        error!("Failed initial IMAP connection: {:?}", e);
        std::io::Error::new(std::io::ErrorKind::Other, format!("IMAP connection failed: {:?}", e))
    })?);
    info!("IMAP connection established successfully (for REST/SSE mode).");

    // --- Create Tool Registry --- 
    let tool_registry_rest = create_mcp_tool_registry(imap_client_rest.clone());
    info!("MCP Tool Registry created.");

    // --- Create Shared State --- 
    let sse_state = Arc::new(TokioMutex::new(SseState::new()));
    info!("SSE shared state initialized.");

    let app_state = AppState::new(imap_client_rest.clone(), tool_registry_rest.clone());
    info!("Application state initialized.");

    let config = web::Data::new(settings.clone());
    info!("Dashboard configuration initialized.");

    let dashboard_state = dashboard::services::init(config.clone(), imap_client_rest.clone());
    info!("Dashboard state initialized.");

    // Start background metrics collection task (needs DashboardState)
    dashboard_state.metrics_service.start_background_collection(dashboard_state.clone());

    // Create and initialize SSE manager
    let sse_manager = Arc::new(SseManager::new(
        Arc::clone(&dashboard_state.metrics_service),
        Arc::clone(&dashboard_state.client_manager)
    ));
    info!("SSE Manager initialized.");

    // --- Start HTTP Server --- 
    let rest_config = settings.rest.as_ref().cloned().unwrap_or_else(|| {
        warn!("No REST configuration found, using defaults");
        rustymail::config::RestConfig {
            enabled: true, // Default to enabled if not in stdio mode
            host: "127.0.0.1".to_string(),
            port: 3000, // Default port
        }
    });
    let (host, port) = (rest_config.host.clone(), rest_config.port);
    let listen_addr = format!("{}:{}", host, port);
    info!("Starting HTTP server (REST & SSE) on {}", listen_addr);

    // Clone state needed for the broadcast task
    let sse_manager_clone_for_task = Arc::clone(&sse_manager);
    let dashboard_state_clone_for_task = dashboard_state.clone();

    let server = HttpServer::new(move || {
        let mut app = App::new()
            // Register all state
            .app_data(web::Data::new(app_state.clone())) 
            .app_data(web::Data::new(sse_state.clone())) 
            .app_data(config.clone())                  
            .app_data(dashboard_state.clone())       
            .app_data(web::Data::new(sse_manager.clone())) // Register SseManager
            .wrap(actix_web::middleware::Logger::default())
            .wrap(dashboard::api::middleware::Metrics) // Re-enable Metrics middleware
            // Configure routes
            .configure(configure_rest_service)       
            .configure(SseAdapter::configure_sse_service)
            .configure(|cfg| dashboard::api::init_routes(cfg)); // Configure dashboard API routes directly

        // Serve static dashboard files (moved from dashboard::init)
        if let Some(dashboard_config) = &config.dashboard {
            if dashboard_config.enabled {
                if let Some(path_str) = &dashboard_config.path {
                    let static_path = std::path::Path::new(path_str);
                    if static_path.exists() {
                        info!("Serving dashboard static files from: {}", path_str);
                        // Clone path_str here for the handler closure
                        let owned_path_str_for_handler = path_str.clone(); 
                        app = app.service(
                            actix_files::Files::new("/dashboard", static_path)
                                .index_file("index.html")
                                .default_handler(
                                    web::get().to(move || { // Closure captures owned_path_str_for_handler
                                        // Clone again for the async move block
                                        let path_for_async = owned_path_str_for_handler.clone(); 
                                        async move {
                                            actix_files::NamedFile::open_async(format!("{}/index.html", path_for_async)).await
                                        }
                                    }),
                                ),
                        );
                    } else {
                        warn!("Dashboard path not found: {}", path_str);
                    }
                }
            }
        }
        app // Return the configured app
    })
    .bind(&listen_addr)
    .map_err(|e| {
        error!("Failed to bind server to {}: {}", listen_addr, e);
        e
    })?
    .run();

    // Spawn the SSE broadcast task 
    info!("Spawning SSE broadcast task...");
    tokio::spawn(async move {
        sse_manager_clone_for_task.start_stats_broadcast(dashboard_state_clone_for_task).await;
    });

    // Await the server
    info!("Server run loop starting.");
    server.await
} 