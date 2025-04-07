use imap_api_rust::config::load_config;
use imap_api_rust::imap::client::{ImapClient, ImapSessionTrait};
use imap_api_rust::api::routes::configure_routes;
use imap_api_rust::models::config::{AppConfig, ImapConfig, ServerConfig};
use actix_web::{
    dev::Service,
    test, web, App,
};
use std::sync::Arc;
use tera::Tera;
use parking_lot::Mutex;
use native_tls::TlsConnector;
use std::net::TcpStream;
use imap as imap_crate;
use crate::mocks::mock::MockImapSession;

pub mod config;

// Helper to load test-specific configuration if needed, or fall back to default
pub fn setup_test_config() -> AppConfig {
    // For now, just use the default loading logic.
    // Could override with specific test config files or env vars later.
    load_config().expect("Failed to load test configuration")
}

// Define the concrete session type
type ActualImapSession = imap_crate::Session<native_tls::TlsStream<TcpStream>>;

pub fn get_test_imap_config() -> ImapConfig {
    ImapConfig {
        host: "localhost".to_string(),
        port: 993,
        username: "test@example.com".to_string(),
        password: "password".to_string(),
    }
}

pub fn get_test_server_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
    }
}

// Helper functions for test data management
pub async fn ensure_test_folder_exists<S: ImapSessionTrait>(
    client: &ImapClient<S>, 
    folder_name: &str
) -> Result<(), Box<dyn std::error::Error>> {
    // Try to create the folder, but don't fail if it already exists
    let _ = client.create_folder(folder_name).await;
    Ok(())
}

pub async fn clean_test_folder<S: ImapSessionTrait>(
    client: &ImapClient<S>, 
    folder_name: &str
) -> Result<(), Box<dyn std::error::Error>> {
    // Try to delete the folder if it exists
    let _ = client.delete_folder(folder_name).await;
    Ok(())
}

pub async fn setup_test_app_data() -> web::Data<Arc<Mutex<ImapClient<MockImapSession>>>> {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    web::Data::new(Arc::new(Mutex::new(client)))
}

pub async fn setup_mock_test_app() -> impl Service<actix_web::dev::ServiceRequest> {
    let app_data = setup_test_app_data().await;
    test::init_service(
        App::new()
            .app_data(app_data.clone())
            .configure(configure_routes::<MockImapSession>)
            .service(web::scope(""))
    ).await
}


