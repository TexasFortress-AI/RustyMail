use imap_api_rust::config::load_config;
use imap_api_rust::imap::client::{ImapClient, ImapSessionTrait};
use imap_api_rust::api::routes::configure_routes;
use imap_api_rust::models::config::AppConfig;
use actix_web::{
    dev::{ServiceFactory, ServiceRequest, ServiceResponse},
    web, App, Error,
};
use std::sync::Arc;
use tera::Tera;

pub mod config;
use config::TestConfig;

// Helper to load test-specific configuration if needed, or fall back to default
pub fn setup_test_config() -> AppConfig {
    // For now, just use the default loading logic.
    // Could override with specific test config files or env vars later.
    load_config().expect("Failed to load test configuration")
}

// Get a real IMAP client for testing using the provided credentials
pub fn get_real_imap_client<S: ImapSessionTrait>() -> Arc<ImapClient<S>> {
    let imap_config = TestConfig {
        imap_server: "p3plzcpnl505455.prod.phx3.secureserver.net".to_string(),
        imap_port: 993,
        imap_username: "info@texasfortress.ai".to_string(),
        imap_password: "M0P@fc9#fy2Kr1TC".to_string(),
        use_tls: true,
    };
    
    Arc::new(ImapClient::new(
        imap_config.imap_server,
        imap_config.imap_port,
        imap_config.imap_username,
        imap_config.imap_password,
    ))
}

// Helper to initialize the Actix App for testing with a real IMAP client
pub fn setup_test_app<S: ImapSessionTrait>() -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse,
        Error = Error,
        InitError = (),
    >,
> {
    let imap_client = get_real_imap_client::<S>();
    // Minimal Tera setup for tests, assuming no templates needed or mock them
    let tera = Tera::new("templates/**/*").unwrap_or_else(|_| Tera::default());

    App::new()
        .app_data(web::Data::new(imap_client.clone()))
        .app_data(web::Data::new(tera.clone()))
        .configure(configure_routes)
        // Add root/docs handlers if they are part of lib.rs or called differently
        // This assumes they are configured within configure_routes or globally
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

// You might add more helpers here, e.g.,
// - Function to ensure a specific test folder exists/is empty
// - Function to create a test email


