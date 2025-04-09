use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::dashboard::api::models::{ClientInfo, ClientType, ClientStatus, PaginatedClients, Pagination};
use log::{debug, warn, info};

#[derive(Debug, Clone)]
pub struct ClientData {
    pub id: String,
    pub client_type: ClientType,
    pub connected_at: DateTime<Utc>,
    pub status: ClientStatus, 
    pub last_activity: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: usize,
}

pub struct ClientManager {
    clients: RwLock<HashMap<String, ClientData>>,
    cleanup_interval: Duration,
}

impl ClientManager {
    pub fn new(cleanup_interval: Duration) -> Self {
        let clients = RwLock::new(HashMap::new());
        let manager = Self {
            clients,
            cleanup_interval,
        };
        
        // Start the cleanup task
        let manager_clone = Arc::new(manager.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                Self::cleanup_inactive_clients(manager_clone.clone()).await;
            }
        });
        
        manager
    }
    
    // Register a new client
    pub async fn register_client(
        &self, 
        client_type: ClientType, 
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> String {
        let client_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        let client_data = ClientData {
            id: client_id.clone(),
            client_type,
            connected_at: now,
            status: ClientStatus::Active,
            last_activity: now,
            ip_address,
            user_agent,
            request_count: 0,
        };
        
        let mut clients = self.clients.write().await;
        clients.insert(client_id.clone(), client_data);
        
        info!("Registered new client: {}", client_id);
        client_id
    }
    
    // Update client activity
    pub async fn update_client_activity(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.last_activity = Utc::now();
            client.request_count += 1;
            debug!("Updated activity for client {}", client_id);
        } else {
            warn!("Attempted to update non-existent client: {}", client_id);
        }
    }
    
    // Update client status
    pub async fn update_client_status(&self, client_id: &str, status: ClientStatus) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.status = status; // No need to clone, it's Copy
            client.last_activity = Utc::now();
            debug!("Updated status for client {} to {:?}", client_id, status);
        } else {
            warn!("Attempted to update status for non-existent client: {}", client_id);
        }
    }
    
    // Remove a client
    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        if clients.remove(client_id).is_some() {
            info!("Removed client: {}", client_id);
        } else {
            warn!("Attempted to remove non-existent client: {}", client_id);
        }
    }
    
    // Get all clients with pagination and optional filtering
    pub async fn get_clients(
        &self,
        page: usize,
        limit: usize,
        filter: Option<&str>,
    ) -> PaginatedClients {
        let clients = self.clients.read().await;
        
        // Filter clients based on the optional filter string
        let filtered_clients: Vec<ClientInfo> = clients
            .values()
            .filter(|client| {
                if let Some(filter_text) = filter {
                    // Apply filter to different fields
                    client.ip_address.as_ref().map_or(false, |ip| ip.contains(filter_text))
                        || client.user_agent.as_ref().map_or(false, |ua| ua.contains(filter_text))
                        || match client.status {
                            ClientStatus::Active if filter_text.eq_ignore_ascii_case("active") => true,
                            ClientStatus::Idle if filter_text.eq_ignore_ascii_case("idle") => true,
                            ClientStatus::Disconnecting if filter_text.eq_ignore_ascii_case("disconnecting") => true,
                            _ => false,
                        }
                } else {
                    true // No filter, include all clients
                }
            })
            .map(|client| ClientInfo {
                id: client.id.clone(),
                r#type: client.client_type,
                connected_at: client.connected_at.to_rfc3339(),
                status: client.status,
                last_activity: client.last_activity.to_rfc3339(),
                ip_address: client.ip_address.clone(),
                user_agent: client.user_agent.clone(),
            })
            .collect();
        
        let total = filtered_clients.len();
        let total_pages = if total == 0 { 
            1 
        } else { 
            (total + limit - 1) / limit 
        };
        
        // Apply pagination
        let page = page.max(1).min(total_pages);
        let offset = (page - 1) * limit;
        let paginated_clients = filtered_clients
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
            
        PaginatedClients {
            clients: paginated_clients,
            pagination: Pagination {
                total,
                page,
                limit,
                total_pages,
            },
        }
    }
    
    // Get client count
    pub async fn get_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }
    
    // Cleanup inactive clients (run periodically)
    async fn cleanup_inactive_clients(manager: Arc<ClientManager>) {
        let now = Utc::now();
        let mut to_remove = Vec::new();
        
        // Find inactive clients
        {
            let clients_read = manager.clients.read().await;
            for (id, client) in clients_read.iter() {
                // If last activity was more than 30 minutes ago, mark for removal
                if (now - client.last_activity).num_minutes() > 30 {
                    to_remove.push(id.clone());
                }
            }
        }
        
        // Remove inactive clients
        if !to_remove.is_empty() {
            let mut clients_write = manager.clients.write().await;
            for id in to_remove.iter() {
                clients_write.remove(id);
                info!("Removed inactive client: {}", id);
            }
            info!("Cleaned up {} inactive clients", to_remove.len());
        }
    }
}

// Implement Clone for ClientManager to allow it to be used in cleanup tasks
impl Clone for ClientManager {
    fn clone(&self) -> Self {
        // Create a new clients hashmap with the same interval
        Self {
            clients: RwLock::new(HashMap::new()),
            cleanup_interval: self.cleanup_interval,
        }
    }
}
