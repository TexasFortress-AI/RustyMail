use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use tokio::sync::RwLock;
use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::debug;
use crate::dashboard::api::models::{DashboardStats, RequestRateData, SystemHealth, SystemStatus};
use std::collections::VecDeque;
use rand;

// Store for metrics data
#[derive(Debug)]
struct MetricsStore {
    active_connections: usize,
    request_rate_points: VecDeque<RequestRateData>,
    cpu_usage: f32,
    memory_usage: f32,
    #[allow(dead_code)] // Keep for potential future uptime calculation
    start_time: Instant,
    last_updated: chrono::DateTime<Utc>,
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self {
            active_connections: 0,
            request_rate_points: VecDeque::with_capacity(24),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            start_time: Instant::now(),
            last_updated: Utc::now(),
        }
    }
}

pub struct MetricsService {
    metrics_store: Arc<RwLock<MetricsStore>>,
    #[allow(dead_code)] // May be used later for dynamic interval adjustment
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
        sys.refresh_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );

        let mut store_guard = store.write().await;

        // System metrics
        store_guard.cpu_usage = sys.global_cpu_info().cpu_usage();
        store_guard.memory_usage = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;

        // TODO: Update request_rate_points (needs tracking mechanism)
        if store_guard.request_rate_points.len() >= 24 {
             store_guard.request_rate_points.pop_front();
        }
        store_guard.request_rate_points.push_back(RequestRateData {
             timestamp: Utc::now().to_rfc3339(),
             value: (rand::random::<u32>() % 100 + 20), // Random placeholder
        });

        // TODO: Collect IMAP active_connections
        
        store_guard.last_updated = Utc::now();
        // Debug log
        debug!("Collected metrics - CPU: {:.1}%, Mem: {:.1}%", store_guard.cpu_usage, store_guard.memory_usage);
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

        DashboardStats {
            active_connections: store.active_connections, 
            request_rate: store.request_rate_points.iter().cloned().collect(),
            system_health: SystemHealth {
                status,
                cpu_usage: store.cpu_usage,
                memory_usage: store.memory_usage,
            },
            last_updated: store.last_updated.to_rfc3339(),
        }
    }
}
