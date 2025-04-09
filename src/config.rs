// use config::ConfigError;
use config::{Environment, File};
use serde::{Deserialize, Serialize};
// Remove unused import
// use std::path::Path;
use thiserror::Error;
// Remove unused env import if not needed elsewhere
// use std::env; 
use std::path::PathBuf; // Import PathBuf
// Remove dotenvy
// use dotenvy; 

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
        // Remove dotenvy loading

        // Determine the configuration file path (default.toml)
        let config_file_path = match config_path {
            Some(p) => PathBuf::from(p),
            None => {
                // Use CARGO_MANIFEST_DIR, handle potential error
                let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                    .map_err(|e| config::ConfigError::Foreign(Box::new(e)))?; 
                let mut default_path = PathBuf::from(manifest_dir);
                default_path.push("config");
                default_path.push("default.toml");
                default_path
            }
        };

        println!("Attempting to load configuration from: {:?}", config_file_path);

        let builder = config::Config::builder()
            // Add default values
            .set_default("interface", "rest")?
            .set_default("log.level", "info")?
            .set_default("dashboard.enabled", false)?
            // Load config file source, but make it optional
            .add_source(File::from(config_file_path.clone()).required(false))
            // Use Environment::default() to load unprefixed variables
            .add_source(Environment::default().separator("__"));
            // Remove manual overrides

        // Build and deserialize
        builder.build()?.try_deserialize()
    }
}

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("Failed to load or parse configuration: {0}")]
    LoadError(#[from] config::ConfigError),
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig { level: "info".to_string() }
    }
} 