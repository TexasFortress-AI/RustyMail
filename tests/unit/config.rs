// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[cfg(test)]
mod tests {
    use rustymail::config::{Settings, InterfaceType};
    use std::env;
    use serial_test::serial;
    use tempfile::TempDir;

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

    // Helper to create a config file in a temp directory
    fn create_config_file(temp_dir: &TempDir, filename: &str, content: &str) -> String {
        let path = temp_dir.path().join(filename);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    #[serial]
    fn test_load_default_config() {
        setup_test_env();
        let temp_dir = TempDir::new().unwrap();

        // Config uses flat fields, not nested [imap] section
        let default_content = r#"
interface = "rest"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "default_user"
imap_pass = "default_pass"

[log]
level = "info"

[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
        let config_path = create_config_file(&temp_dir, "default.toml", default_content);

        let settings = Settings::new(Some(&config_path)).expect("Failed to load default settings");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.log.level, "info");
        // Note: env vars take precedence, so imap_host comes from IMAP_HOST env var
        assert_eq!(settings.imap_user, "default_user");
        // temp_dir will be cleaned up automatically when it goes out of scope
    }

    #[test]
    #[serial]
    fn test_load_custom_config_override() {
        // Set up env vars for this test - set port to 9090 before calling Settings::new
        env::set_var("REST_HOST", "127.0.0.1");
        env::set_var("REST_PORT", "9090"); // This port should take precedence over config file
        env::set_var("SSE_HOST", "127.0.0.1");
        env::set_var("SSE_PORT", "9438");
        env::set_var("DASHBOARD_PORT", "9439");
        env::set_var("RUSTYMAIL_API_KEY", "test-rustymail-key-2024");
        env::set_var("IMAP_HOST", "localhost");
        env::set_var("IMAP_PORT", "143");

        let temp_dir = TempDir::new().unwrap();

        let custom_content = r#"
interface = "rest"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "custom_user"
imap_pass = "custom_pass"

[log]
level = "debug"

[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
        let custom_path = create_config_file(&temp_dir, "custom.toml", custom_content);

        let settings = Settings::new(Some(&custom_path)).expect("Failed to load custom settings");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.imap_user, "custom_user"); // From custom config
        assert_eq!(settings.rest.as_ref().unwrap().port, 9090); // From env var (takes precedence over config's 8080)

        // Restore default for other tests
        env::set_var("REST_PORT", "9437");
        // temp_dir will be cleaned up automatically when it goes out of scope
    }

    #[test]
    #[serial]
    fn test_env_override() {
        setup_test_env();
        let temp_dir = TempDir::new().unwrap();

        let default_content = r#"
interface = "rest"
imap_host = "imap.example.com"
imap_port = 993
imap_user = "default_user"
imap_pass = "default_pass"

[log]
level = "info"

[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
        let config_path = create_config_file(&temp_dir, "default.toml", default_content);

        // Set environment variables that will override config file
        env::set_var("IMAP_PASS", "env_pass");

        let settings = Settings::new(Some(&config_path)).expect("Failed to load settings with env vars");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.imap_pass, "env_pass"); // Env var overrides config
        assert_eq!(settings.imap_user, "default_user"); // From config file

        // Clean up env vars
        env::remove_var("IMAP_PASS");
        // temp_dir will be cleaned up automatically when it goes out of scope
    }
}
