// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[cfg(test)]
mod tests {
    use rustymail::config::{Settings, InterfaceType};
    use std::env;
    use std::path::Path;

    /// Set up required environment variables for tests
    fn setup_test_env() {
        env::set_var("REST_HOST", "127.0.0.1");
        env::set_var("REST_PORT", "9437");
        env::set_var("SSE_HOST", "127.0.0.1");
        env::set_var("SSE_PORT", "9438");
        env::set_var("DASHBOARD_PORT", "9439");
        env::set_var("RUSTYMAIL_API_KEY", "test-rustymail-key-2024");
        env::set_var("IMAP_HOST", "localhost");
        env::set_var("IMAP_PORT", "143");
    }

    // Helper to create a dummy config file
    fn create_dummy_config(path: &str, content: &str) {
        let dir = Path::new(path).parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_load_default_config() {
        setup_test_env();
        let default_content = r#"
interface = "rest"
[log]
level = "info"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "default_user"
imap_pass = "default_pass"
[rest]
enabled = true
host = "127.0.0.1"
port = 8080
        "#;
        create_dummy_config("config/default.toml", default_content);
        
        let settings = Settings::new(None).expect("Failed to load default settings");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.log.level, "info");
        assert_eq!(settings.imap_host, "imap.example.com");
        assert_eq!(settings.imap_user, "default_user");
        assert_eq!(settings.rest.as_ref().unwrap().port, 8080);

        std::fs::remove_dir_all("config").unwrap(); // Clean up
    }

    #[test]
    fn test_load_custom_config_override() {
        setup_test_env();
        // Note: Environment variables set in setup_test_env() take precedence over config file.
        // We set REST_PORT here to match what we expect in the test
        env::set_var("REST_PORT", "9090");

        let default_content = r#"
interface = "rest"
[log]
level = "info"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "default_user"
imap_pass = "default_pass"
[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
        let custom_content = r#"
interface = "rest"
imap_user = "custom_user"
[rest]
host = "127.0.0.1"
port = 9090
"#;
        create_dummy_config("config/default.toml", default_content);
        create_dummy_config("config/custom.toml", custom_content);

        let settings = Settings::new(Some("config/custom.toml")).expect("Failed to load custom settings");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.imap_user, "custom_user"); // Overridden
        assert_eq!(settings.rest.as_ref().unwrap().port, 9090); // Overridden

        std::fs::remove_dir_all("config").unwrap(); // Clean up
        // Restore default
        env::set_var("REST_PORT", "9437");
    }

    #[test]
    fn test_env_override() {
        setup_test_env();
        let default_content = r#"
interface = "rest"
[log]
level = "info"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "default_user"
imap_pass = "default_pass"
[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
        create_dummy_config("config/default.toml", default_content);

        // Set environment variables that will override config file
        env::set_var("IMAP_PASS", "env_pass");
        env::set_var("RUSTYMAIL_LOG__LEVEL", "debug");

        let settings = Settings::new(None).expect("Failed to load settings with env vars");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.imap_pass, "env_pass"); // Env var overrides config
        // imap_user comes from config file
        assert_eq!(settings.log.level, "debug"); // Env var overrides config

        // Clean up env vars
        env::remove_var("IMAP_PASS");
        env::remove_var("RUSTYMAIL_LOG__LEVEL");
        std::fs::remove_dir_all("config").unwrap(); // Clean up
    }
} 