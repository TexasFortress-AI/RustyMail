use rustymail::config::Settings;
use rustymail::api::rest::run_server as run_rest_server;
// Comment out MCP server import for now
// use rustymail::mcp_port::run_mcp_port_server;
// Comment out unused SSE import
// use rustymail::api::sse::SseAdapter;
// Import ImapClient directly
use rustymail::imap::client::ImapClient;
use std::sync::Arc;
use log::{info, error};
use std::process::exit;
// Remove unused imports
// use actix_web::{web, App, HttpServer, Responder, HttpResponse};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Settings::new(None).unwrap_or_else(|err| {
        eprintln!("Failed to load configuration: {}", err);
        exit(1);
    });

    info!("Attempting initial IMAP connection...");
    let host = &config.imap_host;
    let port = config.imap_port;
    let user = &config.imap_user;
    let pass = &config.imap_pass;

    // Call connect and handle the Result directly
    let imap_client_result = ImapClient::connect(host, port, user, pass).await;

    let imap_client = match imap_client_result {
        Ok(client) => {
            info!("IMAP connection and client creation successful.");
            // Wrap the successfully created client in Arc for sharing
            Arc::new(client)
        },
        Err(e) => {
            error!("Initial IMAP connection/client creation failed: {}. Exiting.", e);
            exit(1);
        }
    };

    // --- Start REST Server Directly (if configured) ---
    if let Some(rest_config) = config.rest {
        info!("Starting REST server directly on {}:{}...", rest_config.host, rest_config.port);
        // Pass the Arc<ImapClient> clone
        match run_rest_server(rest_config.clone(), imap_client.clone()).await {
            Ok(_) => info!("REST server finished."),
            Err(e) => error!("REST server failed: {}", e),
        }
    } else {
        info!("REST server is not configured, skipping.");
    }

    // --- MCP / Other Servers (Commented Out) ---
    // If needed, these would likely need to run concurrently using tokio::join! 
    // or similar *before* starting the Actix server if Actix blocks the main thread.
    /*
    if let Some(mcp_config) = config.mcp {
        info!("Starting MCP port server on {}:{}...", mcp_config.host, mcp_config.port);
        let client_clone = imap_client.clone();
        let config_clone = mcp_config.clone();
        tasks.push(tokio::spawn(async move {
            run_mcp_port_server(client_clone, config_clone).await
        }));
    } else {
        info!("MCP port server is not configured, skipping.");
    }
    */

    // Start SSE Adapter/Server (assuming integrated with REST for now)
    // If SSE needs separate setup, add it here.
    // If using SseAdapter::configure_sse_service within run_rest_server, this is handled.

    // Remove task waiting logic as we run REST server directly now
    info!("Main function finished.");
    Ok(())
} 