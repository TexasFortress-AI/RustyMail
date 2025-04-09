use actix_web::{web, App, HttpServer};
use rustymail::config::Settings;
use rustymail::imap::ImapClient;
use rustymail::api::rest::{AppState, configure_rest_service};
use std::sync::Arc;
use dotenvy::dotenv;
use log::{info, debug, error, warn};
use rustymail::mcp_port::create_mcp_tool_registry;
use rustymail::api::mcp_sse::SseAdapter;
use rustymail::api::mcp_sse::SseState;
use tokio::sync::Mutex as TokioMutex;
use env_logger;
use rustymail::dashboard; // Import dashboard module

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

    // Get or create default REST config
    let rest_config = settings.rest.as_ref().cloned().unwrap_or_else(|| {
        warn!("No REST configuration found, using defaults");
        rustymail::config::RestConfig {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 3000
        }
    });
    info!("REST config: enabled={}, host={}, port={}", rest_config.enabled, rest_config.host, rest_config.port);

    // --- Handle Stdio Interface ---
    if active_interface == rustymail::config::InterfaceType::McpStdio {
        info!("Starting in MCP Stdio mode...");
        
        // Create IMAP client for stdio mode
        info!("Connecting to IMAP server (for Stdio mode)...");
        let imap_client_stdio = Arc::new(ImapClient::connect(
            &settings.imap_host,
            settings.imap_port,
            &settings.imap_user,
            &settings.imap_pass,
        ).await.expect("Failed to create IMAP client for Stdio"));
        info!("IMAP connection established successfully (for Stdio mode).");

        // Create tool registry for stdio mode
        let tool_registry_stdio = create_mcp_tool_registry(imap_client_stdio.clone());

        // Create and run the stdio adapter
        let stdio_adapter = rustymail::api::mcp_stdio::McpStdioAdapter::new(tool_registry_stdio);
        
        // Run the adapter and handle potential IO errors
        if let Err(e) = stdio_adapter.run().await {
             error!("MCP Stdio Adapter failed: {}", e);
             return Err(e);
        } else {
            info!("MCP Stdio Adapter finished cleanly.");
            return Ok(()); // Exit cleanly after stdio finishes
        }
    }

    // --- Handle REST/SSE Interface (if not Stdio) ---
    info!("Starting in REST/SSE mode...");
    if !rest_config.enabled {
        panic!("REST interface must be enabled if not in Stdio mode");
    }
    debug!("REST config: host={}, port={}", rest_config.host, rest_config.port);
    debug!("IMAP config: host={}, port={}, user={}", settings.imap_host, settings.imap_port, settings.imap_user);

    // Create IMAP client for REST/SSE mode
    info!("Connecting to IMAP server (for REST/SSE mode)...");
    let imap_client_rest = Arc::new(ImapClient::connect(
        &settings.imap_host,
        settings.imap_port,
        &settings.imap_user,
        &settings.imap_pass,
    ).await.expect("Failed to create IMAP client for REST/SSE"));
    info!("IMAP connection established successfully (for REST/SSE mode).");

    // Create the shared tool registry
    let tool_registry_rest = create_mcp_tool_registry(imap_client_rest.clone());
    info!("MCP Tool Registry created.");

    // Create shared SSE state using the new constructor
    let sse_state = Arc::new(TokioMutex::new(SseState::new()));
    info!("SSE shared state initialized.");

    // Create the application state containing client and registry
    let app_state = AppState::new(imap_client_rest.clone(), tool_registry_rest.clone());
    info!("Application state initialized.");

    // Create shared configuration for dashboard
    let config = web::Data::new(settings.clone());
    info!("Dashboard configuration initialized.");

    // Configure and start HTTP server
    let (host, port) = (rest_config.host, rest_config.port);
    let listen_addr = format!("{}:{}", host, port);
    info!("Starting HTTP server (REST & SSE) on {}", listen_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone())) // Share AppState (client + registry)
            .app_data(web::Data::new(sse_state.clone())) // Share SSE state separately
            .app_data(config.clone()) // Share configuration for dashboard
            .wrap(actix_web::middleware::Logger::default()) // Add logger middleware
            .configure(configure_rest_service) // Configure REST routes
            .configure(SseAdapter::configure_sse_service) // Configure SSE routes
            .configure(|cfg| {
                // Initialize dashboard module with routes and static file serving
                dashboard::init(config.clone(), cfg);
            })
    })
    .bind(&listen_addr)
    .map_err(|e| {
        error!("Failed to bind server to {}: {}", listen_addr, e);
        e
    })?
    .run()
    .await
} 