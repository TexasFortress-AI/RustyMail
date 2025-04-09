use tokio::sync::RwLock;
use crate::dashboard::api::models::{ServerConfig, ImapAdapter};
use log::info;
use std::time::Instant;
use crate::config::Settings;
use sysinfo;

#[derive(Debug, Clone)]
pub struct ConfigData {
    pub active_adapter_id: String,
    pub available_adapters: Vec<ImapAdapter>,
    pub start_time: Instant,
    pub version: String,
}

pub struct ConfigService {
    config: RwLock<ConfigData>,
    current_config: RwLock<Option<Settings>>,
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

        // Load initial config (consider injecting Settings instead)
        let initial_settings = Settings::new(None).ok(); 

        Self {
            config: RwLock::new(config_data),
            current_config: RwLock::new(initial_settings),
        }
    }

    // Get the current server configuration relevant to the dashboard
    pub async fn get_configuration(&self) -> ServerConfig {
        let config_guard = self.current_config.read().await;
        let version = env!("CARGO_PKG_VERSION").to_string();
        // Use sysinfo for system uptime
        let uptime = sysinfo::System::uptime(); 

        if let Some(settings) = &*config_guard {
            // Create adapter list based on settings
            let adapters = vec![
                ImapAdapter {
                    id: "default".to_string(),
                    name: settings.imap_host.clone(),
                    description: format!("IMAP server at {}:{}", settings.imap_host, settings.imap_port),
                    is_active: true,
                }
            ];
            // Since active_adapter is not Option, unwrap the first or provide a default
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
        } else {
            // Return default config if settings failed to load
            let default_adapter = ImapAdapter {
                 id: "unknown".to_string(),
                 name: "Unknown".to_string(),
                 description: "Settings not loaded".to_string(),
                 is_active: true,
            };
            ServerConfig {
                active_adapter: default_adapter,
                available_adapters: vec![],
                version,
                uptime,
            }
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
