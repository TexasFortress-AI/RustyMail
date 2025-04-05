use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub imap: ImapConfig,
    pub server: ServerConfig,
    pub log_level: String,
}
