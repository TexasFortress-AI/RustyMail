// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

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
// --- Add imports for registry ---
// Remove unused imports
// use rustymail::api::mcp_stdio::McpStdioAdapter;
// use rustymail::api::mcp_sse::SseState;  // Not implemented yet

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Settings::new(None).unwrap_or_else(|err| {
        eprintln!("Failed to load configuration: {}", err);
        exit(1);
    });

    // --- Determine Interface (though this binary seems REST-focused) ---
    // We might want to refine this binary's purpose or add logic similar to main.rs
    // For now, assume it always runs REST if configured.

    info!("Attempting initial IMAP connection...");
    let host = &config.imap_host;
    let port = config.imap_port;
    let user = &config.imap_user;
    let pass = &config.imap_pass;

    // Call connect and handle the Result directly
    use rustymail::prelude::AsyncImapSessionWrapper;
    let imap_client_result = ImapClient::<AsyncImapSessionWrapper>::connect(host, port, user, pass).await;

    let imap_client = match imap_client_result {
        Ok(client) => {
            info!("IMAP connection and client creation successful.");
            Arc::new(client)
        },
        Err(e) => {
            error!("Initial IMAP connection/client creation failed: {}. Exiting.", e);
            exit(1);
        }
    };

    // --- Create Tool Registry (disabled for now) ---
    // Tool registry creation needs to be updated for new architecture
    // let tool_registry = create_mcp_tool_registry(...);
    info!("Tool Registry creation disabled - needs update.");

    // --- Initialize SSE State (disabled for now) ---
    // let sse_state = Arc::new(TokioMutex::new(SseState::new()));
    // info!("SSE State initialized.");

    // --- Start REST Server Directly (if configured) ---
    if let Some(ref rest_config) = config.rest {
        if !rest_config.enabled { 
             error!("REST server is configured but not enabled in rustymail binary. Exiting.");
             exit(1);
        }
        info!("Starting REST server directly on {}:{}...", rest_config.host, rest_config.port);
        // Need to create MCP handler and session manager for the server
        use rustymail::mcp::adapters::sdk::SdkMcpAdapter;
        use rustymail::session_manager::SessionManager;

        let mcp_handler: Arc<dyn rustymail::mcp::handler::McpHandler> = Arc::new(
            SdkMcpAdapter::new_placeholder()
                .expect("Failed to create MCP adapter")
        );
        let session_manager = Arc::new(SessionManager::new(Arc::new(config.clone())));

        match run_rest_server(config.clone(), mcp_handler, session_manager, None).await {
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