// use config::ConfigError;
use config::{Environment, File};
use serde::{Deserialize, Serialize};
// Remove unused import
// use std::path::Path;
use thiserror::Error;

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
        let default_config_path = "config/default.toml"; 
        let path_to_use = config_path.unwrap_or(default_config_path);

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
            .add_source(File::with_name(path_to_use).required(false))
            // Environment source expects flattened names like APP_IMAP__HOST now
            .add_source(Environment::with_prefix("APP").separator("__"));

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