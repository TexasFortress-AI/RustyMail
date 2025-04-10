// use config::ConfigError;
use config::{Environment, File};
use serde::{Deserialize, Serialize};
// Remove unused import
// use std::path::Path;
use thiserror::Error;
// Remove unused env import if not needed elsewhere
use std::env;
// Remove dotenvy
// use dotenvy;
use log::warn;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceType {
    Rest,
    McpStdio,
    Sse, // Placeholder for future
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String, 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub path: Option<String>, // Path to static frontend files
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub interface: InterfaceType,
    pub log: LogConfig,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_user: String,
    pub imap_pass: String,
    pub rest: Option<RestConfig>, // Use Option for potentially disabled sections
    pub mcp_stdio: Option<McpStdioConfig>,
    pub dashboard: Option<DashboardConfig>, // Dashboard configuration
}

impl Settings {
    pub fn new(config_path: Option<&str>) -> Result<Self, config::ConfigError> {
        // Default configuration values
        let mut config_builder = config::Config::builder()
            // Default interface value
            .set_default("interface", "rest")?
            
            // IMAP defaults
            .set_default("imap_host", "localhost")?
            .set_default("imap_port", 993)?
            
            // REST defaults
            .set_default("rest.host", "127.0.0.1")?
            .set_default("rest.port", 3000)?
            .set_default("rest.enabled", true)?
            
            // Dashboard defaults
            .set_default("dashboard.enabled", false)?
            // Log defaults
            .set_default("log.level", "info")?;
        
        // Add configuration from file
        if let Some(path) = config_path {
            config_builder = config_builder.add_source(File::with_name(path));
        }
        
        // Add environment variables with prefix
        // e.g. `RUSTYMAIL_IMAP_HOST=...` would override `imap_host`
        config_builder = config_builder.add_source(
            Environment::with_prefix("RUSTYMAIL")
                .separator("_")
                .ignore_empty(true)
        );
        
        // Add direct environment variables for important settings
        // e.g. `IMAP_HOST=...` would override `imap_host`
        let env_vars = [
            ("IMAP_HOST", "imap_host"),
            ("IMAP_PORT", "imap_port"),
            ("IMAP_USER", "imap_user"),
            ("IMAP_PASS", "imap_pass"),
            ("REST_HOST", "rest.host"),
            ("REST_PORT", "rest.port"),
            ("REST_ENABLED", "rest.enabled"),
            ("DASHBOARD_ENABLED", "dashboard.enabled"),
            ("DASHBOARD_PATH", "dashboard.path"),
        ];
        
        for (env_var, config_path) in &env_vars {
            if let Ok(value) = env::var(env_var) {
                // Handle special case for port which needs to be parsed to integer
                if *env_var == "IMAP_PORT" || *env_var == "REST_PORT" {
                    if let Ok(port) = value.parse::<u16>() {
                        config_builder = config_builder.set_override(config_path, port)?;
                    } else {
                        warn!("Invalid port value in {}: {}", env_var, value);
                    }
                } else if *env_var == "DASHBOARD_ENABLED" || *env_var == "REST_ENABLED" {
                    if let Ok(enabled) = value.parse::<bool>() {
                        config_builder = config_builder.set_override(config_path, enabled)?;
                    } else {
                        warn!("Invalid boolean value in {}: {}", env_var, value);
                    }
                } else {
                    config_builder = config_builder.set_override(config_path, value)?;
                }
            }
        }
        
        // Build the config and deserialize it into Settings
        config_builder.build()?.try_deserialize()
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig { level: "info".to_string() }
    }
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

impl Default for McpStdioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
        }
    }
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interface: InterfaceType::Rest,
            log: LogConfig::default(),
            imap_host: "localhost".to_string(),
            imap_port: 993,
            imap_user: String::new(),
            imap_pass: String::new(),
            rest: Some(RestConfig::default()),
            mcp_stdio: Some(McpStdioConfig::default()),
            dashboard: Some(DashboardConfig::default()),
        }
    }
}

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("Failed to load or parse configuration: {0}")]
    LoadError(#[from] config::ConfigError),
} 