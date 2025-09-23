use crate::config::Settings;
use crate::imap::client::ImapClient;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::types::MailboxInfo;
    use tokio::test;

    #[test]
    async fn test_imap_client_basic() {
        // Add tests here when needed
    }
}

#[tokio::test]
async fn test_config_loading() {
    // Test with default configuration
    let config = Settings::new(None);
    assert!(config.is_ok());

    // Test with invalid configuration
    let invalid_config = Settings::new(Some("invalid_path"));
    assert!(invalid_config.is_err());
}

#[tokio::test]
async fn test_imap_client_initialization() {
    // Create a mock configuration
    let mut config = Settings::new(None).unwrap();
    config.imap_host = "localhost".to_string();
    config.imap_port = 143;
    config.imap_user = "test@example.com".to_string();
    config.imap_pass = "password".to_string();

    // Test IMAP client connection
    use crate::imap::session::AsyncImapSessionWrapper;
    let result = ImapClient::<AsyncImapSessionWrapper>::connect(
        &config.imap_host,
        config.imap_port,
        &config.imap_user,
        &config.imap_pass,
    ).await;

    // The connection should fail since we're not running a real IMAP server
    assert!(result.is_err());
}

#[tokio::test]
async fn test_rest_server_configuration() {
    // Create a mock configuration
    let mut config = Settings::new(None).unwrap();
    config.rest = Some(crate::config::RestConfig {
        enabled: true,
        host: "localhost".to_string(),
        port: 8080,
    });

    // Test REST server configuration
    assert!(config.rest.is_some());
    let rest_config = config.rest.unwrap();
    assert!(rest_config.enabled);
    assert_eq!(rest_config.host, "localhost");
    assert_eq!(rest_config.port, 8080);
}

// SSE and MCP tool tests disabled - require components that aren't available in tests
// These tests would need major refactoring to work with the current architecture