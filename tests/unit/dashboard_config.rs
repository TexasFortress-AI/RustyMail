// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Unit tests for dashboard configuration management functionality
#[cfg(test)]
mod tests {
    use rustymail::dashboard::services::config::ConfigService;
    use rustymail::config::Settings;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_config_service_initialization() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings.clone(), None);

        // Test getting settings
        let retrieved_settings = config_service.get_settings().await;
        assert_eq!(retrieved_settings.imap_host, settings.imap_host);
        assert_eq!(retrieved_settings.imap_port, settings.imap_port);
    }

    #[tokio::test]
    async fn test_update_imap_config() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Update IMAP configuration
        let result = config_service.update_imap_config(
            "mail.example.com".to_string(),
            993,
            "user@example.com".to_string(),
            "password123".to_string(),
        ).await;

        assert!(result.is_ok());

        // Verify the update
        let updated_settings = config_service.get_settings().await;
        assert_eq!(updated_settings.imap_host, "mail.example.com");
        assert_eq!(updated_settings.imap_port, 993);
        assert_eq!(updated_settings.imap_user, "user@example.com");
        assert_eq!(updated_settings.imap_pass, "password123");
    }

    #[tokio::test]
    async fn test_update_rest_config() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Update REST configuration
        let result = config_service.update_rest_config(
            true,
            "localhost".to_string(),
            8080,
        ).await;

        assert!(result.is_ok());

        // Verify the update
        let updated_settings = config_service.get_settings().await;
        assert!(updated_settings.rest.is_some());
        let rest_config = updated_settings.rest.unwrap();
        assert!(rest_config.enabled);
        assert_eq!(rest_config.host, "localhost");
        assert_eq!(rest_config.port, 8080);
    }

    #[tokio::test]
    async fn test_update_dashboard_config() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Update dashboard configuration
        let result = config_service.update_dashboard_config(
            true,
            3000,
            Some("/tmp".to_string()),
        ).await;

        assert!(result.is_ok());

        // Verify the update
        let updated_settings = config_service.get_settings().await;
        assert!(updated_settings.dashboard.is_some());
        let dashboard_config = updated_settings.dashboard.unwrap();
        assert!(dashboard_config.enabled);
        assert_eq!(dashboard_config.port, 3000);
        assert_eq!(dashboard_config.path, Some("/tmp".to_string()));
    }

    #[tokio::test]
    async fn test_config_validation() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings.clone(), None);

        // Valid configuration should pass
        let result = config_service.validate_config(&settings).await;
        assert!(result.is_ok());

        // Test with invalid configuration
        let mut invalid_settings = settings.clone();
        invalid_settings.imap_host = "".to_string();
        invalid_settings.imap_port = 0;

        let result = config_service.validate_config(&invalid_settings).await;
        assert!(result.is_err());

        if let Err(errors) = result {
            assert!(errors.iter().any(|e| e.contains("IMAP host cannot be empty")));
            assert!(errors.iter().any(|e| e.contains("IMAP port cannot be 0")));
        }
    }

    #[tokio::test]
    async fn test_invalid_port_validation() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Test port 0 rejection
        let result = config_service.update_imap_config(
            "mail.example.com".to_string(),
            0,
            "user@example.com".to_string(),
            "password".to_string(),
        ).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid port number");
    }

    #[tokio::test]
    async fn test_empty_host_validation() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Test empty host rejection
        let result = config_service.update_imap_config(
            "".to_string(),
            993,
            "user@example.com".to_string(),
            "password".to_string(),
        ).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Host cannot be empty");
    }

    #[tokio::test]
    async fn test_invalid_dashboard_path() {
        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, None);

        // Test non-existent path rejection
        let result = config_service.update_dashboard_config(
            true,
            3000,
            Some("/this/path/does/not/exist/zzz123".to_string()),
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Dashboard path does not exist"));
    }

    #[tokio::test]
    async fn test_config_persistence() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let settings = Settings::default();
        let config_service = ConfigService::with_settings(settings, Some(config_path.clone()));

        // Update configuration
        config_service.update_imap_config(
            "persistent.example.com".to_string(),
            143,
            "persistent@example.com".to_string(),
            "persistpass".to_string(),
        ).await.unwrap();

        // Verify file was created
        assert!(config_path.exists());

        // Read the file and verify contents
        let contents = std::fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("persistent.example.com"));
        assert!(contents.contains("143"));
        assert!(contents.contains("persistent@example.com"));
    }
}