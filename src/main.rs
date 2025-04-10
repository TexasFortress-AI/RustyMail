use actix_web::{web, App, HttpServer};
use rustymail::config::Settings;
// Remove direct ImapClient import if only used for connect check, keep if factory needs it explicitly
// use rustymail::imap::ImapClient; 
use rustymail::api::rest::{AppState, configure_rest_service, ImapSessionFactory}; // Add ImapSessionFactory
use std::sync::Arc;
use dotenvy::dotenv;
use log::{info, error, warn};
// Remove tool registry creation
// use rustymail::mcp_port::create_mcp_tool_registry;
// --- Add McpHandler and SdkMcpAdapter imports ---
use rustymail::mcp::handler::McpHandler;
use rustymail::mcp::adapters::sdk::SdkMcpAdapter;
// --- End imports ---
// --- Replace SseAdapter with configure_sse_service ---
// use rustymail::api::mcp_sse::SseAdapter;
use rustymail::api::mcp_sse::{SseState, configure_sse_service}; // Import the function
// --- End replace ---
use tokio::sync::Mutex as TokioMutex;
use env_logger;
use rustymail::dashboard;
use rustymail::dashboard::api::SseManager; // Keep for dashboard
// --- Add imports for factory --- 
use rustymail::imap::client::ImapClient; // Needed for the factory closure
use rustymail::prelude::{ImapSession, ImapError}; // Needed for factory types
use std::future::Future;
use std::pin::Pin;
// --- End imports for factory --- 
use crate::api::rest::start_rest_api_server;
// use crate::dashboard::start_dashboard_server;
use crate::imap::ImapSessionFactory;

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

    // --- Perform initial IMAP connection check --- (Optional but good for validation)
    info!("Performing initial IMAP connection check...");
    match ImapClient::connect(
        &settings.imap_host,
        settings.imap_port,
        &settings.imap_user,
        &settings.imap_pass,
    ).await {
        Ok(client) => {
            info!("Initial IMAP connection successful. Logging out...");
            // Use try_logout to avoid panicking if logout fails
            if let Err(logout_err) = client.try_logout().await {
                 warn!("Failed to logout after initial connection check: {:?}", logout_err);
            }
        }
        Err(e) => {
            error!("Initial IMAP connection failed: {:?}. Server startup aborted.", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("IMAP connection failed: {:?}", e)));
        }
    }

    // --- Create IMAP Session Factory --- 
    let imap_settings = settings.clone(); // Clone settings needed for the factory
    let imap_session_factory: ImapSessionFactory = Arc::new(move || {
        let settings_clone = imap_settings.clone(); // Clone again for the async block
        Box::pin(async move {
            info!("ImapSessionFactory: Creating new IMAP session..."); // Add log
            let client = ImapClient::connect(
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
    info!("IMAP Session Factory created.");

    // --- Create Tool Registry (REMOVED) --- 
    // let tool_registry_rest = create_mcp_tool_registry(imap_client_rest.clone());
    // info!("MCP Tool Registry created.");

    // --- Create MCP Handler --- 
    // TODO: Implement SdkMcpAdapter::new properly
    let mcp_handler: Arc<dyn McpHandler> = Arc::new(
        SdkMcpAdapter::new()
            .expect("SdkMcpAdapter initialization failed - rmcp SDK integration needed")
    );
    info!("MCP Handler (SdkMcpAdapter) created.");

    // --- Create Shared State --- 
    let sse_state = Arc::new(TokioMutex::new(SseState::new()));
    info!("SSE shared state initialized.");

    // --- Create AppState --- 
    let app_state = AppState::new(imap_session_factory.clone(), mcp_handler.clone(), sse_state.clone());
    info!("Application state initialized.");

    // --- Dashboard Setup (remains largely the same, but might need factory later) --- 
    let config = web::Data::new(settings.clone());
    info!("Dashboard configuration initialized.");

    // Dashboard services initialization might eventually need the factory too,
    // but keep as is for now if it only needs the basic client capabilities during init.
    // If dashboard needs persistent sessions or factory, adjust its init function.
    let dashboard_state = dashboard::services::init(config.clone(), imap_session_factory.clone());
    info!("Dashboard state initialized.");

    // Start background metrics collection task (needs DashboardState)
    dashboard_state.metrics_service.start_background_collection(dashboard_state.clone());

    // Create and initialize SSE manager for dashboard
    let sse_manager = Arc::new(SseManager::new(
        Arc::clone(&dashboard_state.metrics_service),
        Arc::clone(&dashboard_state.client_manager)
    ));
    info!("Dashboard SSE Manager initialized.");
    // --- End Dashboard Setup ---

    // --- Start HTTP Server --- 
    let rest_config = settings.rest.as_ref().cloned().unwrap_or_else(|| {
        warn!("No REST configuration found, using defaults");
        rustymail::config::RestConfig {
            enabled: true, 
            host: "127.0.0.1".to_string(),
            port: 3000, 
        }
    });
    let (host, port) = (rest_config.host.clone(), rest_config.port);
    let listen_addr = format!("{}:{}", host, port);
    info!("Starting HTTP server (REST & MCP SSE) on {}", listen_addr);

    // Clone state needed for the broadcast task
    let sse_manager_clone_for_task = Arc::clone(&sse_manager);
    let dashboard_state_clone_for_task = dashboard_state.clone();

    let server = HttpServer::new(move || {
        let mut app = App::new()
            // --- Register updated state --- 
            .app_data(web::Data::new(app_state.clone()))        // Core AppState (handler, factory, sse_state)
            .app_data(web::Data::new(imap_session_factory.clone())) // Pass factory directly for REST handlers
            .app_data(web::Data::new(sse_state.clone()))        // Pass SseState for MCP SSE handlers
            // --- End updated state --- 
            .app_data(config.clone())                             // Dashboard config
            .app_data(dashboard_state.clone())                  // Dashboard state
            .app_data(web::Data::new(sse_manager.clone()))      // Dashboard SSE Manager
            .wrap(actix_web::middleware::Logger::default())
            .wrap(dashboard::api::middleware::Metrics) 
            // Configure routes
            .configure(configure_rest_service)                // RustyMail REST API
            .configure(configure_sse_service)                 // RustyMail MCP SSE API
            .configure(|cfg| dashboard::api::init_routes(cfg)); // Dashboard API routes

        // Serve static dashboard files (logic remains the same)
        if let Some(dashboard_config) = &config.dashboard {
            if dashboard_config.enabled {
                if let Some(path_str) = &dashboard_config.path {
                    let static_path = std::path::Path::new(path_str);
                    if static_path.exists() {
                        info!("Serving dashboard static files from: {}", path_str);
                        let owned_path_str_for_handler = path_str.clone(); 
                        app = app.service(
                            actix_files::Files::new("/dashboard", static_path)
                                .index_file("index.html")
                                .default_handler(
                                    web::get().to(move || { 
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

    // Spawn the Dashboard SSE broadcast task 
    info!("Spawning Dashboard SSE broadcast task...");
    tokio::spawn(async move {
        sse_manager_clone_for_task.start_stats_broadcast(dashboard_state_clone_for_task).await;
    });

    // Await the server
    info!("Server run loop starting.");
    server.await
} 