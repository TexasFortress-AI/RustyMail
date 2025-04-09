use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::dashboard::api::models::{ServerConfig, ImapAdapter};
use log::{info, error};
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};

#[derive(Debug, Clone)]
pub struct ConfigData {
    pub active_adapter_id: String,
    pub available_adapters: Vec<ImapAdapter>,
    pub start_time: Instant,
    pub version: String,
}

pub struct ConfigService {
    config: RwLock<ConfigData>,
}

impl ConfigService {
    pub fn new() -> Self {
        // Define available adapters
        let available_adapters = vec![
            ImapAdapter {
                id: "mock".to_string(),
                name: "Mock IMAP".to_string(),
                description: "In-memory IMAP server for testing".to_string(),
                is_active: true,
            },
            ImapAdapter {
                id: "godaddy".to_string(),
                name: "GoDaddy".to_string(),
                description: "Live GoDaddy IMAP server".to_string(),
                is_active: false,
            },
        ];

        let config_data = ConfigData {
            active_adapter_id: "mock".to_string(), // Default to mock
            available_adapters,
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        Self {
            config: RwLock::new(config_data),
        }
    }

    // Get the current server configuration
    pub async fn get_configuration(&self) -> ServerConfig {
        let config = self.config.read().await;
        
        // Find the active adapter
        let active_adapter = config.available_adapters.iter()
            .find(|a| a.id == config.active_adapter_id)
            .cloned()
            .unwrap_or_else(|| {
                error!("Active adapter not found in available adapters");
                ImapAdapter {
                    id: "unknown".to_string(),
                    name: "Unknown".to_string(),
                    description: "Unknown adapter".to_string(),
                    is_active: true,
                }
            });

        // Calculate uptime in seconds
        let uptime = config.start_time.elapsed().as_secs();
        
        ServerConfig {
            active_adapter,
            available_adapters: config.available_adapters.clone(),
            version: config.version.clone(),
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
}
