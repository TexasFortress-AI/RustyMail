// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! API Key Authentication Module
//!
//! This module provides API key-based authentication for the REST API.
//! It includes middleware for request validation and API key management.

use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    error::ErrorUnauthorized,
    Error as ActixError, HttpMessage,
};
use actix_web_lab::middleware::Next;
use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::errors::ApiError;

/// API Key metadata and permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique API key string
    pub key: String,
    /// Human-readable name for the API key
    pub name: String,
    /// Associated email address
    pub email: String,
    /// IMAP credentials associated with this API key
    pub imap_credentials: ImapCredentials,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,
    /// Whether the key is active
    pub is_active: bool,
    /// Rate limiting configuration
    pub rate_limit: RateLimit,
    /// Allowed IP addresses (empty = all allowed)
    pub allowed_ips: Vec<String>,
    /// Permissions/scopes
    pub scopes: Vec<ApiScope>,
}

/// IMAP credentials associated with an API key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapCredentials {
    pub username: String,
    pub password: String,
    pub server: String,
    pub port: u16,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Requests per minute
    pub requests_per_minute: u32,
    /// Requests per hour
    pub requests_per_hour: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            requests_per_hour: 1000,
        }
    }
}

/// API permission scopes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApiScope {
    /// Read email and folder information
    ReadEmail,
    /// Modify emails (flags, move, delete)
    WriteEmail,
    /// Create and manage folders
    ManageFolders,
    /// Access dashboard features
    Dashboard,
    /// Admin operations
    Admin,
}

/// API Key store that manages all API keys
#[derive(Debug, Clone)]
pub struct ApiKeyStore {
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Track request counts for rate limiting
    request_counts: Arc<RwLock<HashMap<String, RequestCounter>>>,
}

#[derive(Debug, Clone)]
struct RequestCounter {
    minute_count: u32,
    minute_reset: DateTime<Utc>,
    hour_count: u32,
    hour_reset: DateTime<Utc>,
}

impl ApiKeyStore {
    /// Create a new API key store
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            request_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize API keys from environment variables
    /// Reads RUSTYMAIL_API_KEY to create an admin API key for dashboard/MCP access
    pub async fn init_from_env(&self) {
        if let Ok(api_key) = std::env::var("RUSTYMAIL_API_KEY") {
            if api_key.is_empty() || api_key == "your-secure-api-key-here" {
                warn!("RUSTYMAIL_API_KEY is not configured - API authentication will fail");
                warn!("Generate a secure key with: openssl rand -hex 32");
                return;
            }

            let env_key = ApiKey {
                key: api_key.clone(),
                name: "Environment API Key".to_string(),
                email: "configured-via-env".to_string(),
                imap_credentials: ImapCredentials {
                    username: String::new(),
                    password: String::new(),
                    server: String::new(),
                    port: 0,
                },
                created_at: Utc::now(),
                last_used: None,
                is_active: true,
                rate_limit: RateLimit::default(),
                allowed_ips: vec![],
                scopes: vec![
                    ApiScope::ReadEmail,
                    ApiScope::WriteEmail,
                    ApiScope::ManageFolders,
                    ApiScope::Dashboard,
                    ApiScope::Admin,
                ],
            };

            let mut keys = self.keys.write().await;
            keys.insert(env_key.key.clone(), env_key);
            info!("Initialized API key store from RUSTYMAIL_API_KEY environment variable");
        } else {
            warn!("RUSTYMAIL_API_KEY environment variable not set - API authentication will fail");
            warn!("Set RUSTYMAIL_API_KEY to a secure value (generate with: openssl rand -hex 32)");
        }
    }

    /// Initialize with default API keys for testing only
    /// WARNING: This should only be used in tests, never in production
    #[cfg(test)]
    pub async fn init_with_test_defaults(&self) {
        let test_key = ApiKey {
            key: "test-api-key-12345".to_string(),
            name: "Test API Key".to_string(),
            email: "test@example.com".to_string(),
            imap_credentials: ImapCredentials {
                username: "test@example.com".to_string(),
                password: "test-password".to_string(),
                server: "localhost".to_string(),
                port: 1143,
            },
            created_at: Utc::now(),
            last_used: None,
            is_active: true,
            rate_limit: RateLimit::default(),
            allowed_ips: vec![],
            scopes: vec![
                ApiScope::ReadEmail,
                ApiScope::WriteEmail,
                ApiScope::ManageFolders,
                ApiScope::Dashboard,
            ],
        };

        let mut keys = self.keys.write().await;
        keys.insert(test_key.key.clone(), test_key);
    }

    /// Generate a new API key
    pub async fn create_api_key(
        &self,
        name: String,
        email: String,
        imap_credentials: ImapCredentials,
        scopes: Vec<ApiScope>,
    ) -> String {
        let api_key = format!("rmail_{}", Uuid::new_v4().to_string().replace("-", ""));

        let key_data = ApiKey {
            key: api_key.clone(),
            name,
            email,
            imap_credentials,
            created_at: Utc::now(),
            last_used: None,
            is_active: true,
            rate_limit: RateLimit::default(),
            allowed_ips: vec![],
            scopes,
        };

        let mut keys = self.keys.write().await;
        keys.insert(api_key.clone(), key_data);

        info!("Created new API key: {}", api_key);
        api_key
    }

    /// Validate an API key and check permissions
    pub async fn validate_key(&self, key: &str) -> Result<ApiKey, ApiError> {
        let keys = self.keys.read().await;

        match keys.get(key) {
            Some(api_key) if api_key.is_active => {
                debug!("Valid API key found for: {}", api_key.name);
                Ok(api_key.clone())
            }
            Some(_) => {
                warn!("Inactive API key used: {}", key);
                Err(ApiError::InvalidApiKey { reason: "API key is inactive".to_string() })
            }
            None => {
                warn!("Unknown API key: {}", key);
                Err(ApiError::InvalidApiKey { reason: "Invalid API key".to_string() })
            }
        }
    }

    /// Check if API key has specific scope
    pub async fn has_scope(&self, key: &str, scope: &ApiScope) -> bool {
        if let Ok(api_key) = self.validate_key(key).await {
            return api_key.scopes.contains(scope);
        }
        false
    }

    /// Check rate limits for an API key
    pub async fn check_rate_limit(&self, key: &str) -> Result<(), ApiError> {
        let api_key = self.validate_key(key).await?;
        let mut counters = self.request_counts.write().await;

        let now = Utc::now();
        let counter = counters.entry(key.to_string()).or_insert_with(|| {
            RequestCounter {
                minute_count: 0,
                minute_reset: now + chrono::Duration::minutes(1),
                hour_count: 0,
                hour_reset: now + chrono::Duration::hours(1),
            }
        });

        // Reset counters if time windows have passed
        if now > counter.minute_reset {
            counter.minute_count = 0;
            counter.minute_reset = now + chrono::Duration::minutes(1);
        }
        if now > counter.hour_reset {
            counter.hour_count = 0;
            counter.hour_reset = now + chrono::Duration::hours(1);
        }

        // Check rate limits
        if counter.minute_count >= api_key.rate_limit.requests_per_minute {
            return Err(ApiError::RateLimitExceeded { message: "API key rate limit exceeded (per minute)".to_string() });
        }
        if counter.hour_count >= api_key.rate_limit.requests_per_hour {
            return Err(ApiError::RateLimitExceeded { message: "API key rate limit exceeded (per hour)".to_string() });
        }

        // Increment counters
        counter.minute_count += 1;
        counter.hour_count += 1;

        Ok(())
    }

    /// Update last used timestamp for an API key
    pub async fn update_last_used(&self, key: &str) {
        let mut keys = self.keys.write().await;
        if let Some(api_key) = keys.get_mut(key) {
            api_key.last_used = Some(Utc::now());
        }
    }

    /// Check if request IP is allowed for the API key
    pub async fn check_ip_restriction(&self, key: &str, ip: Option<&str>) -> Result<(), ApiError> {
        let api_key = self.validate_key(key).await?;

        // If no IP restrictions, allow all
        if api_key.allowed_ips.is_empty() {
            return Ok(());
        }

        // Check if IP is in allowed list
        if let Some(client_ip) = ip {
            if api_key.allowed_ips.contains(&client_ip.to_string()) {
                return Ok(());
            }
        }

        Err(ApiError::Unauthorized)
    }

    /// Get API key metadata (without sensitive info)
    pub async fn get_key_info(&self, key: &str) -> Result<ApiKeyInfo, ApiError> {
        let api_key = self.validate_key(key).await?;

        Ok(ApiKeyInfo {
            name: api_key.name,
            email: api_key.email,
            created_at: api_key.created_at,
            last_used: api_key.last_used,
            is_active: api_key.is_active,
            scopes: api_key.scopes,
        })
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, key: &str) -> Result<(), ApiError> {
        let mut keys = self.keys.write().await;

        match keys.get_mut(key) {
            Some(api_key) => {
                api_key.is_active = false;
                info!("Revoked API key: {}", key);
                Ok(())
            }
            None => Err(ApiError::NotFound { resource: "API key".to_string() })
        }
    }

    /// Get IMAP credentials for an API key
    pub async fn get_imap_credentials(&self, key: &str) -> Result<ImapCredentials, ApiError> {
        let api_key = self.validate_key(key).await?;
        Ok(api_key.imap_credentials)
    }
}

/// Public API key information (no sensitive data)
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub scopes: Vec<ApiScope>,
}

/// Enhanced API key validation middleware
pub async fn validate_api_key_enhanced<B>(
    req: ServiceRequest,
    next: Next<B>,
    store: Arc<ApiKeyStore>,
) -> Result<ServiceResponse<B>, ActixError>
where
    B: actix_web::body::MessageBody,
{
    // Extract API key from headers
    let api_key = req.headers()
        .get("X-API-Key")
        .or_else(|| req.headers().get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .map(|s| {
            // Handle "Bearer " prefix if present
            if s.starts_with("Bearer ") {
                &s[7..]
            } else {
                s
            }
        });

    let api_key = match api_key {
        Some(key) => key,
        None => {
            warn!("Request missing API key");
            return Err(ErrorUnauthorized("Missing API key"));
        }
    };

    // Validate the API key
    let api_key_data = store.validate_key(api_key).await
        .map_err(|_| ErrorUnauthorized("Invalid API key"))?;

    // Check IP restrictions
    let client_ip = req.peer_addr().map(|addr| addr.ip().to_string());
    store.check_ip_restriction(api_key, client_ip.as_deref()).await
        .map_err(|_| ErrorUnauthorized("IP not allowed"))?;

    // Check rate limits
    store.check_rate_limit(api_key).await
        .map_err(|e| ErrorUnauthorized(format!("Rate limit exceeded: {}", e)))?;

    // Update last used timestamp
    store.update_last_used(api_key).await;

    // Store API key data in request extensions for later use
    req.extensions_mut().insert(api_key_data);

    // Continue with the request
    next.call(req).await
}

/// Simple validation middleware that stores API key in app data
pub async fn simple_validate_api_key(
    req: ServiceRequest,
    next: Next<impl actix_web::body::MessageBody>,
) -> Result<ServiceResponse<impl actix_web::body::MessageBody>, ActixError> {
    // Check for API key in header
    let api_key = req.headers()
        .get("X-API-Key")
        .or_else(|| req.headers().get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .map(|s| {
            if s.starts_with("Bearer ") {
                &s[7..]
            } else {
                s
            }
        });

    let api_key = match api_key {
        Some(key) => key,
        None => {
            warn!("Request missing API key");
            return Err(actix_web::error::ErrorUnauthorized("Missing API key"));
        }
    };

    // Get the API key store from app state
    let state = req.app_data::<actix_web::web::Data<crate::api::rest::AppState>>();

    if let Some(app_state) = state {
        // Validate the key
        match app_state.api_key_store.validate_key(api_key).await {
            Ok(_) => {
                // Store API key in request for later retrieval
                // Note: We'll pass it through headers for simplicity
                next.call(req).await
            }
            Err(e) => {
                warn!("Invalid API key: {}", e);
                Err(actix_web::error::ErrorUnauthorized("Invalid API key"))
            }
        }
    } else {
        Err(actix_web::error::ErrorInternalServerError("Server configuration error"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_creation() {
        let store = ApiKeyStore::new();

        let key = store.create_api_key(
            "Test Key".to_string(),
            "test@example.com".to_string(),
            ImapCredentials {
                username: "test".to_string(),
                password: "pass".to_string(),
                server: "localhost".to_string(),
                port: 993,
            },
            vec![ApiScope::ReadEmail],
        ).await;

        assert!(key.starts_with("rmail_"));

        // Validate the created key
        let result = store.validate_key(&key).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let store = ApiKeyStore::new();
        store.init_with_test_defaults().await;

        let test_key = "test-api-key-12345";

        // Should succeed for first requests
        for _ in 0..10 {
            let result = store.check_rate_limit(test_key).await;
            assert!(result.is_ok());
        }

        // Eventually should hit rate limit
        // (In real implementation, would need to adjust rate limits or time)
    }

    #[tokio::test]
    async fn test_scope_checking() {
        let store = ApiKeyStore::new();

        let key = store.create_api_key(
            "Limited Key".to_string(),
            "limited@example.com".to_string(),
            ImapCredentials {
                username: "test".to_string(),
                password: "pass".to_string(),
                server: "localhost".to_string(),
                port: 993,
            },
            vec![ApiScope::ReadEmail],
        ).await;

        assert!(store.has_scope(&key, &ApiScope::ReadEmail).await);
        assert!(!store.has_scope(&key, &ApiScope::Admin).await);
    }
}