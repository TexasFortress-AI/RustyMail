use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Instant, Duration};
use crate::dashboard::api::models::{ServerConfig, ImapAdapter};
use crate::config::Settings;
use log::{info, debug, error};

pub struct ConfigService {
    config: RwLock<ServerConfig>,
    start_time: Instant,
}

impl ConfigService {
    pub fn new(settings: Arc<Settings>) -> Self {
        let imap_host = settings.imap_host.clone();
        let version = env!("CARGO_PKG_VERSION").to_string();
        
        // Create default adapters
        let adapters = vec![
            ImapAdapter {
                id: "default".to_string(),
                name: format!("{} (Default)", imap_host),
                description: format!("Default IMAP server at {}", imap_host),
                is_active: true,
            },
            ImapAdapter {
                id: "mock".to_string(),
                name: "Mock IMAP Server".to_string(),
                description: "Simulated IMAP server for testing".to_string(),
                is_active: false,
            },
            ImapAdapter {
                id: "gmail".to_string(),
                name: "Gmail IMAP".to_string(), 
                description: "Gmail IMAP server (imap.gmail.com)".to_string(),
                is_active: false,
            },
            ImapAdapter {
                id: "outlook".to_string(), 
                name: "Outlook IMAP".to_string(),
                description: "Outlook IMAP server (outlook.office365.com)".to_string(),
                is_active: false,
            }
        ];
        
        let config = ServerConfig {
            active_adapter: adapters[0].clone(),
            available_adapters: adapters,
            version,
            uptime: 0,
        };
        
        Self {
            config: RwLock::new(config),
            start_time: Instant::now(),
        }
    }
    
    // Get current server configuration
    pub async fn get_configuration(&self) -> ServerConfig {
        let mut config = self.config.write().await;
        
        // Update uptime
        config.uptime = self.start_time.elapsed().as_secs();
        
        config.clone()
    }
    
    // Set active IMAP adapter
    pub async fn set_active_adapter(&self, adapter_id: &str) -> Result<ServerConfig, String> {
        let mut config = self.config.write().await;
        
        // Find adapter by ID
        let adapter_index = config.available_adapters
            .iter()
            .position(|a| a.id == adapter_id)
            .ok_or_else(|| format!("Adapter with ID '{}' not found", adapter_id))?;
        
        // Reset all adapters to inactive
        for adapter in &mut config.available_adapters {
            adapter.is_active = false;
        }
        
        // Set selected adapter to active
        config.available_adapters[adapter_index].is_active = true;
        config.active_adapter = config.available_adapters[adapter_index].clone();
        
        // Update uptime
        config.uptime = self.start_time.elapsed().as_secs();
        
        info!("Active IMAP adapter changed to '{}'", adapter_id);
        Ok(config.clone())
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
        config.uptime = self.start_time.elapsed().as_secs();
        
        Ok(config.clone())
    }
    
    // Remove an IMAP adapter
    pub async fn remove_adapter(&self, adapter_id: &str) -> Result<ServerConfig, String> {
        let mut config = self.config.write().await;
        
        // Cannot remove active adapter
        if config.active_adapter.id == adapter_id {
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
        config.uptime = self.start_time.elapsed().as_secs();
        
        Ok(config.clone())
    }
}
