use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use tokio::sync::RwLock;
use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::debug;
use crate::dashboard::api::models::{DashboardStats, SystemHealth, SystemStatus};
use std::collections::VecDeque;

// Store for metrics data
#[derive(Debug)]
struct MetricsStore {
    active_connections: usize, // Changed back to usize
    cpu_usage: f32,
    memory_usage: f32,
    #[allow(dead_code)] // Keep for potential future uptime calculation
    start_time: Instant,
    last_updated: chrono::DateTime<Utc>,
    // Store timestamps of requests within the last minute (or other interval)
    request_timestamps: VecDeque<Instant>,
    // Store response times for requests within the last minute
    response_times_ms: VecDeque<u128>,
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self {
            active_connections: 0, // Initialize usize
            cpu_usage: 0.0,
            memory_usage: 0.0,
            start_time: Instant::now(),
            last_updated: Utc::now(),
            request_timestamps: VecDeque::with_capacity(1000), // Estimate capacity
            response_times_ms: VecDeque::with_capacity(1000), 
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
        if store_guard.request_timestamps.len() >= 24 {
             store_guard.request_timestamps.pop_front();
        }
        store_guard.request_timestamps.push_back(Instant::now());

        // TODO: Collect IMAP active_connections
        
        store_guard.last_updated = Utc::now();
        // Debug log
        debug!("Collected metrics - CPU: {:.1}%, Mem: {:.1}%", store_guard.cpu_usage, store_guard.memory_usage);
    }
    
    pub async fn update_connection_count(&self, count: usize) {
        let mut store_guard = self.metrics_store.write().await;
        store_guard.active_connections = count;
    }
    
    pub async fn increment_connections(&self) {
        let mut store_guard = self.metrics_store.write().await;
        store_guard.active_connections += 1;
    }
    
    pub async fn decrement_connections(&self) {
        let mut store_guard = self.metrics_store.write().await;
        if store_guard.active_connections > 0 {
            store_guard.active_connections -= 1;
        }
    }
    
    pub async fn get_current_stats(&self) -> DashboardStats {
        // Read store data
        let store = self.metrics_store.read().await;
        
        // Calculate Requests Per Minute (RPM)
        let now = Instant::now();
        let cutoff = now.checked_sub(Duration::from_secs(60)).unwrap_or(now); 
        let requests_in_last_minute = store.request_timestamps.iter().filter(|ts| **ts >= cutoff).count();
        let requests_per_minute = requests_in_last_minute as f64; 

        // Calculate Average Response Time
        let total_response_time_ms: u128 = store.response_times_ms.iter().sum();
        let response_count = store.response_times_ms.len();
        let average_response_time_ms = if response_count > 0 {
             total_response_time_ms as f64 / response_count as f64 
        } else {
            0.0
        };

        // Determine system health status 
        let status = if store.cpu_usage > 90.0 || store.memory_usage > 90.0 {
            SystemStatus::Critical 
        } else if store.cpu_usage > 70.0 || store.memory_usage > 70.0 {
            SystemStatus::Degraded 
        } else {
            SystemStatus::Healthy 
        };

        DashboardStats {
            active_connections: store.active_connections, // Read directly
            requests_per_minute, 
            average_response_time_ms, 
            system_health: SystemHealth {
                status,
                cpu_usage: store.cpu_usage, 
                memory_usage: store.memory_usage, 
            },
            last_updated: store.last_updated.to_rfc3339(),
        }
    }

    // Method to be called when a request starts
    pub async fn record_request_start(&self) {
        let mut store = self.metrics_store.write().await;
        let now = Instant::now();
        store.request_timestamps.push_back(now);
        // Prune old timestamps (e.g., older than 60 seconds)
        let cutoff = now - Duration::from_secs(60);
        while let Some(ts) = store.request_timestamps.front() {
            if *ts < cutoff {
                store.request_timestamps.pop_front();
            } else {
                break;
            }
        }
    }

    // Method to be called when a request finishes, with its duration
    pub async fn record_response_time(&self, duration: Duration) {
        let mut store = self.metrics_store.write().await;
        let now = Instant::now();
        let duration_ms = duration.as_millis();
        store.response_times_ms.push_back(duration_ms);
        // Prune old response times (e.g., older than 60 seconds)
        // We need a way to associate response times with timestamps or just keep a fixed window
        // For simplicity, let's prune based on count for now, keeping last N entries
        const MAX_RESPONSE_TIMES: usize = 1000; 
        while store.response_times_ms.len() > MAX_RESPONSE_TIMES {
            store.response_times_ms.pop_front();
        }
        // Also prune request timestamps to avoid unbounded growth if response isn't recorded
        let cutoff = now - Duration::from_secs(60);
         while let Some(ts) = store.request_timestamps.front() {
            if *ts < cutoff {
                store.request_timestamps.pop_front();
            } else {
                break;
            }
        }
    }
}
