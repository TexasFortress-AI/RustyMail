// use config::ConfigError;
use config::{Environment, File};
use serde::{Deserialize, Serialize};
// Remove unused import
// use std::path::Path;
use thiserror::Error;
use std::env; // Import env module
use std::path::PathBuf; // Import PathBuf
use dotenvy; // Import dotenvy crate

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
pub struct Settings {
    pub interface: InterfaceType,
    pub log: LogConfig,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_user: String,
    pub imap_pass: String,
    pub rest: Option<RestConfig>, // Use Option for potentially disabled sections
    pub mcp_stdio: Option<McpStdioConfig>,
}

impl Settings {
    pub fn new(config_path: Option<&str>) -> Result<Self, config::ConfigError> {
        // Construct path to .env relative to Cargo.toml
        let dotenv_path = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(manifest_dir) => {
                let mut path = PathBuf::from(manifest_dir);
                path.push(".env");
                path
            }
            Err(_) => PathBuf::from(".env"), // Fallback if manifest dir not found (e.g., deployed)
        };

        // Load .env file from the explicit path. Ignore error if not found.
        println!("Attempting to load .env from: {:?}", dotenv_path);
        dotenvy::from_path(&dotenv_path).ok();

        // Determine the configuration file path (default.toml)
        let config_file_path = match config_path {
            Some(p) => PathBuf::from(p),
            None => {
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
            // Remove placeholder IMAP defaults - rely on Env Vars
            // .set_default("imap.host", "localhost")? 
            // .set_default("imap.port", 993)?
            // .set_default("imap.user", "user")?
            // .set_default("imap.pass", "pass")?
            // Load sources (File is optional, Env overrides defaults)
            .add_source(File::from(config_file_path.clone()).required(true))
            // Environment source expects flattened names like APP__IMAP__HOST
            .add_source(Environment::with_prefix("APP").separator("__"))
            // Add environment variable overrides (RUSTYMAIL prefix)
            .add_source(Environment::with_prefix("RUSTYMAIL").separator("_"));

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