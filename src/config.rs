use serde::{Deserialize, Serialize};
use config::{Environment, File};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InterfaceType {
    Rest,
    McpStdio,
    Sse, // Placeholder for future
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConnectConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
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
    pub imap: ImapConnectConfig,
    pub rest: Option<RestConfig>, // Use Option for potentially disabled sections
    pub mcp_stdio: Option<McpStdioConfig>,
}

impl Settings {
    pub fn new(config_path: Option<&str>) -> Result<Self, config::ConfigError> {
        let default_config_path = "./config/default.toml";
        let path_to_use = config_path.unwrap_or(default_config_path);

        let builder = config::Config::builder()
            .add_source(File::with_name(path_to_use).required(true))
            .add_source(Environment::with_prefix("APP").separator("__"));

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