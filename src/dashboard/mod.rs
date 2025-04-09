// Dashboard module for RustyMail SSE Dashboard
//
// This module contains all dashboard-related functionality including:
// - API endpoints for the frontend
// - Services for metrics, client management, and AI
// - SSE implementation for real-time updates

pub mod api;
pub mod services;

// Re-export commonly used types
pub use api::models::{DashboardStats, ClientInfo, PaginatedClients};
pub use services::metrics::MetricsService;
pub use services::clients::ClientManager;

// Only include testing in debug builds
#[cfg(test)]
pub mod testing;

use crate::config::Settings;
use actix_web::web;
use std::path::Path;
use actix_files::Files;
use log::{info, warn};

// Initialize the dashboard module and register routes
pub fn init(config: web::Data<Settings>, app_cfg: &mut web::ServiceConfig) {
    info!("Initializing dashboard module");
    
    // Initialize services and get shared state
    let dashboard_state = services::init(config.clone());
    
    // Register API routes with access to dashboard state
    app_cfg.app_data(dashboard_state.clone());
    api::init(app_cfg, dashboard_state.clone());
    
    // Serve static frontend files if dashboard is enabled and path is configured
    if let Some(dashboard_config) = &config.dashboard {
        if dashboard_config.enabled {
            if let Some(path) = &dashboard_config.path {
                let path_clone = path.clone(); // Clone the path for use in the closure
                if Path::new(path).exists() {
                    info!("Serving dashboard static files from: {}", path);
                    app_cfg.service(
                        Files::new("/dashboard", path)
                            .index_file("index.html")
                            .default_handler(
                                web::get().to(move || {
                                    let path_string = path_clone.clone();
                                    async move {
                                        actix_files::NamedFile::open_async(format!("{}/index.html", path_string)).await
                                    }
                                }),
                            ),
                    );
                } else {
                    warn!("Dashboard path not found: {}", path);
                }
            }
        }
    }
}
