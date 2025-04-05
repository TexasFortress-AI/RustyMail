use actix_web::{web, App, HttpServer};
use actix_web::middleware::Logger;
use env_logger::Env;
use std::sync::{Arc, Mutex};
use native_tls::{TlsConnector, TlsStream};
use ::imap as imap_crate;
use crate::imap::client::{ImapClient, ImapSessionTrait};
use crate::api::routes::configure_routes;
use std::net::TcpStream;

mod api;
mod error;
mod imap;
mod models;
mod utils;

// Define the concrete session type
type ActualImapSession = imap_crate::Session<TlsStream<TcpStream>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .expect("Failed to build TLS connector");

    // Connect to IMAP server
    let client = imap_crate::connect(("p3plzcpnl505455.prod.phx3.secureserver.net", 993), "p3plzcpnl505455.prod.phx3.secureserver.net", &tls)
        .expect("Failed to connect to IMAP server");

    // Login to IMAP server
    let imap_session = client
        .login("info@texasfortress.ai", "M0P@fc9#fy2Kr1TC")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Login failed: {}", e.0)))?;

    // Wrap the actual session in Arc<Mutex<ActualImapSession>> which implements ImapSessionTrait
    let session_arc_mutex: Arc<Mutex<ActualImapSession>> = Arc::new(Mutex::new(imap_session));

    // Create the ImapClient with the Arc<Mutex<ActualImapSession>>
    // Note: ImapClient::new now expects Arc<S>, so we clone the Arc here.
    let imap_client_struct = ImapClient::new(session_arc_mutex.clone());

    // Wrap the ImapClient struct itself in app_data
    let app_data = web::Data::new(imap_client_struct);

    println!("Starting server at http://127.0.0.1:8080");

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(app_data.clone())
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
