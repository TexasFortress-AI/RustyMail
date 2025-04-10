#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Helper to create a dummy config file
    fn create_dummy_config(path: &str, content: &str) {
        let dir = Path::new(path).parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_load_default_config() {
        let default_content = r#"
interface = "rest"
[log]
level = "info"
[imap]
host = "imap.example.com"
port = 993
user = "default_user"
pass = "default_pass"
[rest]
enabled = true
host = "127.0.0.1"
port = 8080
[mcp_stdio]
enabled = false
        "#;
        create_dummy_config("config/default.toml", default_content);
        
        let settings = Settings::new(None).expect("Failed to load default settings");

        assert!(matches!(settings.interface, InterfaceType::Rest));
        assert_eq!(settings.log.level, "info");
        assert_eq!(settings.imap.host, "imap.example.com");
        assert_eq!(settings.imap.user, "default_user");
        assert_eq!(settings.rest.as_ref().unwrap().port, 8080);
        assert!(settings.mcp_stdio.as_ref().unwrap().enabled == false);

        std::fs::remove_dir_all("config").unwrap(); // Clean up
    }

    #[test]
    fn test_load_custom_config_override() {
         let default_content = r#"
interface = "rest"
[log]
level = "info"
[imap]
host = "imap.example.com"
port = 993
user = "default_user"
pass = "default_pass"
[rest]
enabled = true
host = "127.0.0.1"
port = 8080
"#;
         let custom_content = r#"
interface = "mcp_stdio" # Override interface
[imap]
user = "custom_user" # Override user
[rest]
port = 9090 # Override port
"#;
        create_dummy_config("config/default.toml", default_content);
        create_dummy_config("config/custom.toml", custom_content);

        let settings = Settings::new(Some("config/custom.toml")).expect("Failed to load custom settings");

        assert!(matches!(settings.interface, InterfaceType::McpStdio));
        assert_eq!(settings.imap.user, "custom_user"); // Overridden
        assert_eq!(settings.imap.host, "imap.example.com"); // From default
        assert_eq!(settings.rest.as_ref().unwrap().port, 9090); // Overridden
        assert!(settings.mcp_stdio.is_none()); // Not defined in custom or default explicitly merged

        std::fs::remove_dir_all("config").unwrap(); // Clean up
    }

    #[test]
    fn test_env_override() {
        let default_content = r#"
interface = "rest"
[log]
level = "info"
[imap]
host = "imap.example.com"
port = 993
user = "default_user"
pass = "default_pass"
"#;
        create_dummy_config("config/default.toml", default_content);

        // Set environment variables
        env::set_var("RUSTYMAIL_INTERFACE", "mcp_stdio");
        env::set_var("RUSTYMAIL_IMAP__PASS", "env_pass"); // Note: use __ for nesting
        env::set_var("RUSTYMAIL_LOG__LEVEL", "debug");

        let settings = Settings::new(None).expect("Failed to load settings with env vars");

        assert!(matches!(settings.interface, InterfaceType::McpStdio));
        assert_eq!(settings.imap.pass, "env_pass");
        assert_eq!(settings.imap.user, "default_user");
        assert_eq!(settings.log.level, "debug");

        // Clean up env vars
        env::remove_var("RUSTYMAIL_INTERFACE");
        env::remove_var("RUSTYMAIL_IMAP__PASS");
        env::remove_var("RUSTYMAIL_LOG__LEVEL");
        std::fs::remove_dir_all("config").unwrap(); // Clean up
    }
} 