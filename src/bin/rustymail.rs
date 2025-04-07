use rustymail::config::{Settings, ImapConnectConfig};
use rustymail::imap::{ImapClient, ImapSession, ImapError};
// Remove API imports for now
// use rustymail::api::rest::run_server as run_rest_server;
// use rustymail::api::mcp::McpStdioAdapter;
// Remove native_tls
// use native_tls::TlsConnector;
use log::{info, error, debug, LevelFilter};
use std::sync::Arc;
use std::time::Duration;

// Simplified helper to directly return the session Arc
async fn connect_to_imap(config: &ImapConnectConfig) -> Result<Arc<dyn ImapSession>, ImapError> {
    info!(
        "Connecting to IMAP: {}@{}:{}",
        config.user,
        config.host,
        config.port
    );
    // Call ImapClient::connect directly
    ImapClient::connect(
        &config.host,
        config.port,
        &config.user,
        &config.pass,
        Some(Duration::from_secs(15)), // Example timeout
    ).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> { // Correct async typo
    // Load Settings
    let settings = Settings::new(None).expect("Failed to load configuration.");
    
    // Initialize logging
    env_logger::Builder::new()
        .filter_level(settings.log.level.parse().unwrap_or(LevelFilter::Info))
        .init();

    debug!("Loaded settings: {:?}", settings);

    // Connect to IMAP - use the simplified helper
    info!("Establishing IMAP connection...");
    let imap_session = match connect_to_imap(&settings.imap).await {
        Ok(session) => {
            info!("IMAP connection successful.");
            session
        }
        Err(e) => {
            error!("Failed to connect to IMAP: {}", e);
            // Keep the explicit cast
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    // Comment out interface selection for now
    /*
    match settings.interface {
        InterfaceType::Rest => {
            if let Some(rest_config) = settings.rest {
                if rest_config.enabled {
                    info!("Starting REST API server at {}:{}...", rest_config.host, rest_config.port);
                    // Placeholder: Need API implementation
                    // let imap_client_for_rest = Arc::new(ImapClient::new(imap_session.clone()));
                    // run_rest_server(rest_config, imap_client_for_rest).await?;
                    error!("REST API not implemented yet.");
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
                    // let imap_client_for_mcp = Arc::new(ImapClient::new(imap_session.clone()));
                    // let adapter = McpStdioAdapter::new(imap_client_for_mcp);
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
            error!("SSE interface not implemented yet.");
        }
    }
    */

    info!("Binary finished (no interface selected/implemented).");
    // Example usage: List folders (using the established session)
    info!("Listing folders...");
    match imap_session.list_folders().await {
        Ok(folders) => {
            info!("Folders found:");
            for folder in folders {
                info!("- {}", folder.name);
            }
        }
        Err(e) => {
            error!("Failed to list folders: {}", e);
        }
    }

    // Explicitly logout
    info!("Logging out...");
    // Call logout directly on the Arc<dyn ImapSession>
    match imap_session.logout().await {
        Ok(_) => info!("Logout successful."),
        Err(e) => error!("Logout failed: {}", e),
    }

    Ok(())
} 