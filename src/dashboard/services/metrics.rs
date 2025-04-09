use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use tokio::sync::RwLock;
use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::debug;
use crate::dashboard::api::models::{DashboardStats, RequestRateData, SystemHealth, SystemStatus};
use std::collections::VecDeque;

// Store for metrics data
#[derive(Debug)]
pub struct MetricsStore {
    pub active_connections: usize,
    pub request_rate_points: VecDeque<RequestRateData>,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub start_time: Instant,
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self {
            active_connections: 0,
            request_rate_points: VecDeque::new(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            start_time: Instant::now(),
        }
    }
}

pub struct MetricsService {
    metrics_store: Arc<RwLock<MetricsStore>>,
    collection_interval: Duration,
}

impl MetricsService {
    // Static initialization function
    pub fn init() {
        debug!("Initializing metrics service");
        // In a real implementation, this would set up the service
        // and potentially store it in a global registry
    }
    
    pub fn new(collection_interval: Duration) -> Self {
        let metrics_store = Arc::new(RwLock::new(MetricsStore::default()));
        
        let store_clone = Arc::clone(&metrics_store);
        
        // Spawn background task to collect metrics
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(collection_interval);
            // Initialize system with specific refresh kinds for CPU and memory
            let refresh_kind = RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything());
            let mut sys = System::new_with_specifics(refresh_kind);
            
            loop {
                interval.tick().await;
                MetricsService::collect_metrics(&mut sys, store_clone.clone()).await;
            }
        });
        
        Self { 
            metrics_store,
            collection_interval,
        }
    }
    
    async fn collect_metrics(sys: &mut System, store: Arc<RwLock<MetricsStore>>) {
        // Refresh system info
        sys.refresh_all();
        
        // Get CPU usage as percentage (0-100)
        let cpu_usage = sys.global_cpu_info().cpu_usage();
        
        // Get memory usage as percentage
        let total_memory = sys.total_memory() as f32;
        let used_memory = sys.used_memory() as f32;
        let memory_usage = if total_memory > 0.0 {
            (used_memory / total_memory) * 100.0
        } else {
            0.0
        };
        
        // Add current request rate data point with timestamp
        let timestamp = Utc::now().to_rfc3339();

        // Update metrics store
        let mut store = store.write().await;
        
        // Maintain up to 24 data points (e.g., last 2 hours if collecting every 5 minutes)
        if store.request_rate_points.len() >= 24 {
            store.request_rate_points.pop_front();
        }
        
        // For now, we're just generating a random number of requests per time period
        // In a real implementation, this would come from actual tracking
        let requests_count = rand::random::<u32>() % 100 + 20; // Random value between 20-120
        
        store.request_rate_points.push_back(RequestRateData {
            timestamp,
            value: requests_count,
        });
        
        // Update other metrics
        store.cpu_usage = cpu_usage;
        store.memory_usage = memory_usage;
        
        // In a real implementation, we would:
        // 1. Count actual active IMAP connections
        // 2. Track request rates from API/MCP calls
        // 3. Add more detailed metrics
        
        debug!("Collected metrics: CPU: {:.1}%, Memory: {:.1}%, Active Connections: {}", 
               cpu_usage, memory_usage, store.active_connections);
    }
    
    pub async fn update_connection_count(&self, count: usize) {
        let mut store = self.metrics_store.write().await;
        store.active_connections = count;
    }
    
    pub async fn increment_connections(&self) {
        let mut store = self.metrics_store.write().await;
        store.active_connections += 1;
    }
    
    pub async fn decrement_connections(&self) {
        let mut store = self.metrics_store.write().await;
        if store.active_connections > 0 {
            store.active_connections -= 1;
        }
    }
    
    pub async fn get_current_stats(&self) -> DashboardStats {
        let store = self.metrics_store.read().await;
        
        // Determine system health status based on CPU and memory usage
        let status = if store.cpu_usage > 90.0 || store.memory_usage > 90.0 {
            SystemStatus::Critical
        } else if store.cpu_usage > 70.0 || store.memory_usage > 70.0 {
            SystemStatus::Degraded
        } else {
            SystemStatus::Healthy
        };
        
        // Convert request rate points to Vec
        let request_rate: Vec<RequestRateData> = store.request_rate_points
            .iter()
            .cloned()
            .collect();
        
        // Get current timestamp in ISO format
        let last_updated = Utc::now().to_rfc3339();
        
        DashboardStats {
            active_connections: store.active_connections,
            request_rate,
            system_health: SystemHealth {
                status,
                cpu_usage: store.cpu_usage,
                memory_usage: store.memory_usage,
            },
            last_updated,
        }
    }
}
