use rustymail::config::{Settings, InterfaceType};
use rustymail::imap::{ImapClient, ImapSession, ImapError};
use rustymail::api::rest::run_server as run_rest_server;
// Remove API imports for now
// use rustymail::api::rest::run_server as run_rest_server;
// use rustymail::api::mcp::McpStdioAdapter;
// Remove native_tls
// use native_tls::TlsConnector;
use log::{info, error, LevelFilter};
use std::sync::Arc;
// Remove unused imports
// use std::time::Duration;

// Define the setup_logging function
fn setup_logging(level_str: &str) {
    let level = level_str.parse().unwrap_or(LevelFilter::Info);
    env_logger::Builder::new()
        .filter_level(level)
        .init();
    info!("Logging initialized with level: {}", level);
}

// Helper function (assuming one exists or needs creating)
async fn connect_to_imap(host: &str, port: u16, user: &str, pass: &str) -> Result<Arc<dyn ImapSession>, ImapError> {
    // Use the provided arguments directly
    ImapClient::connect(host, port, user, pass, None).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    // Load settings
    let settings = Settings::new(None).expect("Failed to load settings");

    // Setup logging (using settings.log.level)
    setup_logging(&settings.log.level);

    // --- IMAP Connection ---
    // Pass flattened fields to the connection helper
    let imap_session = match connect_to_imap(
        &settings.imap_host, 
        settings.imap_port, 
        &settings.imap_user, 
        &settings.imap_pass
    ).await {
        Ok(session) => {
            log::info!("Successfully connected to IMAP server for user {}", settings.imap_user);
            session
        }
        Err(e) => {
            error!("Failed to connect to IMAP: {}", e);
            // Keep the explicit cast
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    let imap_client = Arc::new(ImapClient::new(imap_session.clone()));

    // Uncomment interface selection
    match settings.interface {
        InterfaceType::Rest => {
            if let Some(rest_config) = settings.rest {
                if rest_config.enabled {
                    info!("Starting REST API server at {}:{}...", rest_config.host, rest_config.port);
                    // Uncomment call to run_rest_server
                    run_rest_server(rest_config, imap_client.clone()).await?;
                } else {
                    info!("REST interface is disabled in config.");
                }
            } else {
                info!("REST configuration missing.");
            }
        }
        InterfaceType::McpStdio => {
             if let Some(mcp_config) = settings.mcp_stdio {
                if mcp_config.enabled {
                    info!("Starting MCP Stdio interface...");
                    // Placeholder: Need API implementation
                    // let adapter = McpStdioAdapter::new(imap_client.clone());
                    // adapter.run().await?;
                    error!("MCP Stdio not implemented yet.");
                } else {
                    info!("MCP Stdio interface is disabled in config.");
                }
            } else {
                 info!("MCP Stdio configuration missing.");
            }
        }
        InterfaceType::Sse => {
             // Placeholder: Need API implementation for SSE
            error!("SSE interface not implemented yet.");
        }
    }

    // Remove example folder listing and logout if server runs indefinitely
    // info!("Binary finished (no interface selected/implemented).");
    // ... (remove folder list and logout code) ...

    Ok(())
} 