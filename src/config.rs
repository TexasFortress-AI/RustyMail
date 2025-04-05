use crate::error::ImapApiError;
use crate::models::config::AppConfig;
use config::{Config, Environment, File};
use std::env;

pub fn load_config() -> Result<AppConfig, ImapApiError> {
    let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

    let settings = Config::builder()
        // Start with default values
        // Add configuration files based on RUN_MODE
        .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
        // Add default config file
        .add_source(File::with_name("config/default").required(false))
        // Add environment variables with a prefix of `APP`
        // E.g. `APP_IMAP__PASSWORD="secret"` would set `config.imap.password`
        .add_source(Environment::with_prefix("APP").separator("__"))
        .build()?;

    let app_config: AppConfig = settings.try_deserialize().map_err(|e| {
        ImapApiError::ConfigError(format!("Failed to deserialize config: {}", e))
    })?;

    Ok(app_config)
}
