use actix_web::{web, App, HttpServer};
use rustymail::config::Settings;
use rustymail::imap::ImapClient;
use rustymail::api::rest::{AppState, configure_rest_service};
use std::sync::Arc;
use dotenvy::dotenv;
use log::{info, debug, error};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present
    dotenv().ok();

    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    info!("Starting RustyMail server...");

    // Load configuration
    info!("Loading configuration...");
    let settings = Settings::new(None).expect("Failed to load settings");
    let rest_config = settings.rest.as_ref().expect("REST config section missing or disabled");
    if !rest_config.enabled {
        panic!("REST interface must be enabled");
    }
    debug!("REST config: host={}, port={}", rest_config.host, rest_config.port);
    debug!("IMAP config: host={}, port={}, user={}", settings.imap_host, settings.imap_port, settings.imap_user);

    // Create IMAP client
    info!("Connecting to IMAP server...");
    let imap_client = Arc::new(ImapClient::connect(
        &settings.imap_host,
        settings.imap_port,
        &settings.imap_user,
        &settings.imap_pass,
    ).await.expect("Failed to create IMAP client"));
    info!("IMAP connection established successfully.");

    let app_state = AppState::new(imap_client);
    info!("Application state initialized.");

    // Configure and start HTTP server
    let listen_addr = format!("{}:{}", rest_config.host, rest_config.port);
    info!("Starting REST server on {}", listen_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .configure(configure_rest_service)
    })
    .bind(&listen_addr)
    .map_err(|e| {
        error!("Failed to bind server to {}: {}", listen_addr, e);
        e
    })?
    .run()
    .await
} 