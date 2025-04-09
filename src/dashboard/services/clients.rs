use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;
use log::{info, debug, warn};
use crate::dashboard::api::models::{ClientInfo, ClientType, ClientStatus, PaginatedClients, Pagination};

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
    clients: Arc<RwLock<HashMap<String, ClientData>>>,
    cleanup_interval: Duration,
}

impl ClientManager {
    pub fn new(cleanup_interval: Duration) -> Self {
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let manager = Self {
            clients: clients.clone(),
            cleanup_interval,
        };

        // Start the cleanup task
        let clients_for_task = clients.clone();
        let interval_for_task = cleanup_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval_for_task);
            loop {
                interval.tick().await;
                Self::cleanup_inactive_clients(clients_for_task.clone()).await;
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
        
        let filtered_clients: Vec<ClientInfo> = clients
            .values()
            .filter(|client| {
                match filter {
                    Some(f) if !f.is_empty() => {
                        // Case-insensitive filtering on ID, IP, User Agent, or Status
                        let f_lower = f.to_lowercase();
                        client.id.to_lowercase().contains(&f_lower) ||
                        client.ip_address.as_deref().unwrap_or("").to_lowercase().contains(&f_lower) ||
                        client.user_agent.as_deref().unwrap_or("").to_lowercase().contains(&f_lower) ||
                        format!("{:?}", client.status).to_lowercase().contains(&f_lower)
                    },
                    _ => true, // No filter or empty filter matches all
                }
            })
            .map(|client| ClientInfo {
                id: client.id.clone(),
                r#type: client.client_type.clone(),
                status: client.status.clone(),
                ip_address: client.ip_address.clone(),
                user_agent: client.user_agent.clone(),
                connected_at: client.connected_at.to_rfc3339(),
                last_activity: client.last_activity.to_rfc3339(),
            })
            .collect();
        
        let total = filtered_clients.len();
        let offset = (page.saturating_sub(1)) * limit;
        
        let paginated_data = filtered_clients
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
            
        PaginatedClients {
            clients: paginated_data,
            pagination: Pagination {
                total,
                page,
                limit,
                total_pages: (total as f64 / limit as f64).ceil() as usize,
            }
        }
    }
    
    // Get client count
    pub async fn get_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }
    
    // Cleanup inactive clients (run periodically)
    async fn cleanup_inactive_clients(clients_arc: Arc<RwLock<HashMap<String, ClientData>>>) {
        let now = Utc::now();
        let mut to_remove = Vec::new();

        // Find inactive clients
        {
            let clients_read = clients_arc.read().await;
            for (id, client) in clients_read.iter() {
                // If last activity was more than 30 minutes ago, mark for removal
                let duration = now.signed_duration_since(client.last_activity);
                if duration.num_minutes() > 30 {
                    to_remove.push(id.clone());
                }
            }
        }

        // Remove inactive clients
        if !to_remove.is_empty() {
            let mut clients_write = clients_arc.write().await;
            for id in to_remove.iter() {
                clients_write.remove(id);
                info!("Removed inactive client: {}", id);
            }
            info!("Cleaned up {} inactive clients", to_remove.len());
        }
    }
}
