use std::sync::Arc;
use actix_web::middleware::Logger;
use env_logger::Env;

// Port interfaces
use rustymail::transport::Transport;
use rustymail::config::{Config, Interface};
use rustymail::imap::client::ImapClient;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Load configuration
    let config = Config::load().expect("Failed to load configuration");

    // Initialize IMAP client (core domain)
    let imap_client = Arc::new(ImapClient::new(&config.imap)?);

    // Start appropriate interface based on configuration
    match config.interface {
        Interface::Rest => {
            println!("Starting REST API server on {}:{}", config.rest.host, config.rest.port);
            rustymail::api::rest::start_server(imap_client, &config.rest).await
        }
        Interface::Stdio => {
            println!("Starting MCP stdio server");
            rustymail::api::mcp::start_stdio_server(imap_client).await
        }
        Interface::Sse => {
            println!("Starting SSE server on {}:{}", config.sse.host, config.sse.port);
            rustymail::api::sse::start_server(imap_client, &config.sse).await
        }
    }
} 