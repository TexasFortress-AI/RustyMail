// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use tokio::sync::RwLock;
use crate::dashboard::api::models::{ServerConfig, ImapAdapter};
use log::{info, error};
use std::time::Instant;
use crate::config::Settings;
use sysinfo;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ConfigData {
    pub active_adapter_id: String,
    pub available_adapters: Vec<ImapAdapter>,
    pub start_time: Instant,
    pub version: String,
}

pub struct ConfigService {
    config: RwLock<ConfigData>,
    current_config: Arc<RwLock<Settings>>,
    config_path: Option<PathBuf>,
}

impl ConfigService {
    pub fn new() -> Self {
        Self::with_settings(Settings::new(None).unwrap_or_else(|e| {
            error!("Failed to load settings, using defaults: {}", e);
            Settings::default()
        }), None)
    }

    pub fn with_settings(settings: Settings, config_path: Option<PathBuf>) -> Self {
        // Define available adapters based on current settings
        let available_adapters = vec![
            ImapAdapter {
                id: "current".to_string(),
                name: settings.imap_host.clone(),
                description: format!("IMAP server at {}:{}", settings.imap_host, settings.imap_port),
                is_active: true,
            },
            ImapAdapter {
                id: "mock".to_string(),
                name: "Mock IMAP".to_string(),
                description: "In-memory IMAP server for testing".to_string(),
                is_active: false,
            },
        ];

        let config_data = ConfigData {
            active_adapter_id: "current".to_string(),
            available_adapters,
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        Self {
            config: RwLock::new(config_data),
            current_config: Arc::new(RwLock::new(settings)),
            config_path,
        }
    }

    // Get the current server configuration relevant to the dashboard
    pub async fn get_configuration(&self) -> ServerConfig {
        let settings = self.current_config.read().await;
        let version = env!("CARGO_PKG_VERSION").to_string();
        // Use sysinfo for system uptime
        let uptime = sysinfo::System::uptime();

        // Create adapter list based on settings
        let adapters = vec![
            ImapAdapter {
                id: "current".to_string(),
                name: settings.imap_host.clone(),
                description: format!("IMAP server at {}:{}", settings.imap_host, settings.imap_port),
                is_active: true,
            }
        ];

        let active_adapter = adapters.first().cloned().unwrap_or_else(|| ImapAdapter {
             id: "unknown".to_string(),
             name: "Unknown".to_string(),
             description: "No adapter configured".to_string(),
             is_active: true,
        });

        ServerConfig {
            active_adapter,
            available_adapters: adapters,
            version,
            uptime,
        }
    }

    // Set the active IMAP adapter
    pub async fn set_active_adapter(&self, adapter_id: &str) -> Result<ServerConfig, String> {
        let mut config = self.config.write().await;
        
        // Check if adapter exists
        if !config.available_adapters.iter().any(|a| a.id == adapter_id) {
            return Err(format!("Adapter '{}' not found", adapter_id));
        }

        // Update adapter active status
        for adapter in &mut config.available_adapters {
            adapter.is_active = adapter.id == adapter_id;
        }

        // Update active adapter ID
        config.active_adapter_id = adapter_id.to_string();
        
        info!("Active IMAP adapter set to: {}", adapter_id);
        
        // Return updated configuration
        let active_adapter = config.available_adapters.iter()
            .find(|a| a.id == adapter_id)
            .cloned()
            .unwrap();
            
        let uptime = config.start_time.elapsed().as_secs();
        
        Ok(ServerConfig {
            active_adapter,
            available_adapters: config.available_adapters.clone(),
            version: config.version.clone(),
            uptime,
        })
    }

    // Add a new IMAP adapter
    pub async fn add_adapter(&self, adapter: ImapAdapter) -> Result<ServerConfig, String> {
        let mut config = self.config.write().await;
        
        // Check if adapter with same ID already exists
        if config.available_adapters.iter().any(|a| a.id == adapter.id) {
            return Err(format!("Adapter with ID '{}' already exists", adapter.id));
        }
        
        // Add new adapter
        config.available_adapters.push(adapter);
        
        // Update uptime
        config.start_time = Instant::now();
        
        Ok(self.get_configuration().await)
    }
    
    // Remove an IMAP adapter
    pub async fn remove_adapter(&self, adapter_id: &str) -> Result<ServerConfig, String> {
        let mut config = self.config.write().await;
        
        // Cannot remove active adapter
        if config.active_adapter_id == adapter_id {
            return Err("Cannot remove active adapter".to_string());
        }
        
        // Find adapter index
        let adapter_index = config.available_adapters
            .iter()
            .position(|a| a.id == adapter_id)
            .ok_or_else(|| format!("Adapter with ID '{}' not found", adapter_id))?;
        
        // Remove adapter
        config.available_adapters.remove(adapter_index);
        
        // Update uptime
        config.start_time = Instant::now();
        
        Ok(self.get_configuration().await)
    }

    // Get current settings
    pub async fn get_settings(&self) -> Settings {
        self.current_config.read().await.clone()
    }

    // Update IMAP configuration at runtime
    pub async fn update_imap_config(
        &self,
        host: String,
        port: u16,
        user: String,
        pass: String,
    ) -> Result<(), String> {
        // Validate port
        if port == 0 {
            return Err("Invalid port number".to_string());
        }

        // Validate host
        if host.is_empty() {
            return Err("Host cannot be empty".to_string());
        }

        // Update settings
        let mut settings = self.current_config.write().await;
        settings.imap_host = host.clone();
        settings.imap_port = port;
        settings.imap_user = user.clone();
        settings.imap_pass = pass;

        // Persist if we have a config path
        if let Some(config_path) = &self.config_path {
            if let Err(e) = self.persist_settings(&*settings, config_path).await {
                error!("Failed to persist configuration: {}", e);
                return Err(format!("Failed to save configuration: {}", e));
            }
        }

        info!("IMAP configuration updated: {}:{} (user: {})", host, port, user);
        Ok(())
    }

    // Update REST API configuration
    pub async fn update_rest_config(&self, enabled: bool, host: String, port: u16) -> Result<(), String> {
        if port == 0 {
            return Err("Invalid port number".to_string());
        }

        let mut settings = self.current_config.write().await;
        settings.rest = Some(crate::config::RestConfig {
            enabled,
            host: host.clone(),
            port,
        });

        // Persist if we have a config path
        if let Some(config_path) = &self.config_path {
            if let Err(e) = self.persist_settings(&*settings, config_path).await {
                error!("Failed to persist configuration: {}", e);
                return Err(format!("Failed to save configuration: {}", e));
            }
        }

        info!("REST configuration updated: {} ({}:{})", if enabled { "enabled" } else { "disabled" }, host, port);
        Ok(())
    }

    // Update dashboard configuration
    pub async fn update_dashboard_config(&self, enabled: bool, port: u16, path: Option<String>) -> Result<(), String> {
        if port == 0 {
            return Err("Invalid port number".to_string());
        }

        // Validate path if provided
        if let Some(ref p) = path {
            let path = std::path::Path::new(p);
            if !path.exists() {
                return Err(format!("Dashboard path does not exist: {}", p));
            }
        }

        let mut settings = self.current_config.write().await;
        settings.dashboard = Some(crate::config::DashboardConfig {
            enabled,
            port,
            path: path.clone(),
        });

        // Persist if we have a config path
        if let Some(config_path) = &self.config_path {
            if let Err(e) = self.persist_settings(&*settings, config_path).await {
                error!("Failed to persist configuration: {}", e);
                return Err(format!("Failed to save configuration: {}", e));
            }
        }

        info!("Dashboard configuration updated: {} (port: {})", if enabled { "enabled" } else { "disabled" }, port);
        Ok(())
    }

    // Persist settings to file
    async fn persist_settings(&self, settings: &Settings, path: &PathBuf) -> Result<(), String> {
        let toml_string = toml::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        tokio::fs::write(path, toml_string)
            .await
            .map_err(|e| format!("Failed to write configuration file: {}", e))?;

        info!("Configuration persisted to {:?}", path);
        Ok(())
    }

    // Validate configuration
    pub async fn validate_config(&self, settings: &Settings) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate IMAP settings
        if settings.imap_host.is_empty() {
            errors.push("IMAP host cannot be empty".to_string());
        }
        if settings.imap_port == 0 {
            errors.push("IMAP port cannot be 0".to_string());
        }
        if settings.imap_user.is_empty() {
            errors.push("IMAP user cannot be empty".to_string());
        }

        // Validate REST settings if enabled
        if let Some(rest) = &settings.rest {
            if rest.enabled {
                if rest.host.is_empty() {
                    errors.push("REST host cannot be empty when enabled".to_string());
                }
                if rest.port == 0 {
                    errors.push("REST port cannot be 0".to_string());
                }
            }
        }

        // Validate dashboard settings if enabled
        if let Some(dashboard) = &settings.dashboard {
            if dashboard.enabled {
                if dashboard.port == 0 {
                    errors.push("Dashboard port cannot be 0".to_string());
                }
                if let Some(ref path) = dashboard.path {
                    if !std::path::Path::new(path).exists() {
                        errors.push(format!("Dashboard path does not exist: {}", path));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
