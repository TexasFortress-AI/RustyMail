// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Rate Limiting Middleware
//!
//! Provides request rate limiting with IP-based and global limits.
//! Adds standard X-RateLimit-* headers to all responses.

use actix_web::{
    dev::{ServiceRequest, ServiceResponse, Transform, Service},
    http::header::{HeaderName, HeaderValue},
    Error, HttpResponse,
};
use futures_util::future::{ok, Ready, LocalBoxFuture};
use log::{debug, warn};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::RwLock;

/// Rate limit configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute per IP
    pub per_ip_per_minute: u32,
    /// Requests per hour per IP
    pub per_ip_per_hour: u32,
    /// IPs that bypass rate limiting (comma-separated in env var)
    pub whitelist_ips: Vec<String>,
}

impl RateLimitConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let per_ip_per_minute = std::env::var("RATE_LIMIT_PER_IP_MINUTE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);

        let per_ip_per_hour = std::env::var("RATE_LIMIT_PER_IP_HOUR")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        let whitelist_ips = std::env::var("RATE_LIMIT_WHITELIST_IPS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            per_ip_per_minute,
            per_ip_per_hour,
            whitelist_ips,
        }
    }
}

/// Request counter for tracking rate limits
#[derive(Debug, Clone)]
struct RequestCounter {
    minute_count: u32,
    minute_reset: chrono::DateTime<chrono::Utc>,
    hour_count: u32,
    hour_reset: chrono::DateTime<chrono::Utc>,
}

/// Shared rate limiter state
#[derive(Clone)]
pub struct RateLimiterState {
    config: RateLimitConfig,
    ip_counters: Arc<RwLock<HashMap<String, RequestCounter>>>,
}

impl RateLimiterState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check rate limit for an IP, returns (remaining, reset_timestamp) or error message
    async fn check_and_increment(&self, ip: &str) -> Result<(u32, i64), String> {
        // Check whitelist
        if self.config.whitelist_ips.contains(&ip.to_string()) {
            return Ok((self.config.per_ip_per_minute, 0));
        }

        let mut counters = self.ip_counters.write().await;
        let now = chrono::Utc::now();

        let counter = counters.entry(ip.to_string()).or_insert_with(|| {
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

        // Check limits
        if counter.minute_count >= self.config.per_ip_per_minute {
            let retry_after = (counter.minute_reset - now).num_seconds().max(1);
            return Err(format!(
                "Rate limit exceeded: {} requests per minute. Retry after {} seconds.",
                self.config.per_ip_per_minute, retry_after
            ));
        }
        if counter.hour_count >= self.config.per_ip_per_hour {
            let retry_after = (counter.hour_reset - now).num_seconds().max(1);
            return Err(format!(
                "Rate limit exceeded: {} requests per hour. Retry after {} seconds.",
                self.config.per_ip_per_hour, retry_after
            ));
        }

        // Increment counters
        counter.minute_count += 1;
        counter.hour_count += 1;

        let remaining = self.config.per_ip_per_minute.saturating_sub(counter.minute_count);
        let reset = counter.minute_reset.timestamp();

        Ok((remaining, reset))
    }

    /// Get current rate limit info without incrementing (for adding headers to responses)
    async fn get_limit_info(&self, ip: &str) -> (u32, u32, i64) {
        let counters = self.ip_counters.read().await;

        if let Some(counter) = counters.get(ip) {
            let remaining = self.config.per_ip_per_minute.saturating_sub(counter.minute_count);
            let reset = counter.minute_reset.timestamp();
            (self.config.per_ip_per_minute, remaining, reset)
        } else {
            let reset = (chrono::Utc::now() + chrono::Duration::minutes(1)).timestamp();
            (self.config.per_ip_per_minute, self.config.per_ip_per_minute, reset)
        }
    }
}

/// Rate limiting middleware factory
pub struct RateLimitMiddleware {
    state: RateLimiterState,
}

impl RateLimitMiddleware {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            state: RateLimiterState::new(config),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimitMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimitMiddlewareService {
            service: Rc::new(service),
            state: self.state.clone(),
        })
    }
}

/// Rate limiting middleware service
pub struct RateLimitMiddlewareService<S> {
    service: Rc<S>,
    state: RateLimiterState,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let state = self.state.clone();
        let service = Rc::clone(&self.service);

        // Extract client IP - check proxy headers first, then peer address
        let client_ip = extract_client_ip(&req);
        debug!("Rate limit check for IP: {}", client_ip);

        Box::pin(async move {
            // Check rate limit BEFORE calling the service
            match state.check_and_increment(&client_ip).await {
                Ok((remaining, reset)) => {
                    // Request allowed - call the inner service
                    let mut res = service.call(req).await?;

                    // Add rate limit headers to response
                    let headers = res.headers_mut();
                    let limit = state.config.per_ip_per_minute;

                    if let Ok(val) = HeaderValue::from_str(&limit.to_string()) {
                        headers.insert(
                            HeaderName::from_static("x-ratelimit-limit"),
                            val,
                        );
                    }
                    if let Ok(val) = HeaderValue::from_str(&remaining.to_string()) {
                        headers.insert(
                            HeaderName::from_static("x-ratelimit-remaining"),
                            val,
                        );
                    }
                    if let Ok(val) = HeaderValue::from_str(&reset.to_string()) {
                        headers.insert(
                            HeaderName::from_static("x-ratelimit-reset"),
                            val,
                        );
                    }

                    Ok(res.map_into_left_body())
                }
                Err(message) => {
                    // Rate limit exceeded - return 429 immediately
                    warn!("Rate limit exceeded for IP {}: {}", client_ip, message);

                    let (limit, _, reset) = state.get_limit_info(&client_ip).await;
                    let retry_after = (reset - chrono::Utc::now().timestamp()).max(1);

                    let response = HttpResponse::TooManyRequests()
                        .insert_header(("X-RateLimit-Limit", limit.to_string()))
                        .insert_header(("X-RateLimit-Remaining", "0"))
                        .insert_header(("X-RateLimit-Reset", reset.to_string()))
                        .insert_header(("Retry-After", retry_after.to_string()))
                        .json(serde_json::json!({
                            "error": "rate_limit_exceeded",
                            "message": message,
                            "retry_after": retry_after
                        }));

                    Ok(req.into_response(response).map_into_right_body())
                }
            }
        })
    }
}

/// Extract client IP from request, checking proxy headers first
fn extract_client_ip(req: &ServiceRequest) -> String {
    // Check X-Forwarded-For header (may contain multiple IPs, first is the client)
    if let Some(xff) = req.headers().get("X-Forwarded-For") {
        if let Ok(xff_str) = xff.to_str() {
            if let Some(first_ip) = xff_str.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }

    // Check X-Real-IP header
    if let Some(xri) = req.headers().get("X-Real-IP") {
        if let Ok(ip) = xri.to_str() {
            return ip.trim().to_string();
        }
    }

    // Fall back to peer address
    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let config = RateLimitConfig {
            per_ip_per_minute: 10,
            per_ip_per_hour: 100,
            whitelist_ips: vec![],
        };
        let state = RateLimiterState::new(config);

        // First 10 requests should succeed
        for _ in 0..10 {
            assert!(state.check_and_increment("192.168.1.1").await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let config = RateLimitConfig {
            per_ip_per_minute: 2,
            per_ip_per_hour: 100,
            whitelist_ips: vec![],
        };
        let state = RateLimiterState::new(config);

        // First 2 requests succeed
        assert!(state.check_and_increment("192.168.1.1").await.is_ok());
        assert!(state.check_and_increment("192.168.1.1").await.is_ok());

        // Third request should fail
        assert!(state.check_and_increment("192.168.1.1").await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_whitelist_bypass() {
        let config = RateLimitConfig {
            per_ip_per_minute: 1,
            per_ip_per_hour: 1,
            whitelist_ips: vec!["127.0.0.1".to_string()],
        };
        let state = RateLimiterState::new(config);

        // Whitelisted IP should always succeed
        for _ in 0..100 {
            assert!(state.check_and_increment("127.0.0.1").await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_isolates_ips() {
        let config = RateLimitConfig {
            per_ip_per_minute: 2,
            per_ip_per_hour: 100,
            whitelist_ips: vec![],
        };
        let state = RateLimiterState::new(config);

        // Exhaust limit for IP 1
        state.check_and_increment("192.168.1.1").await.unwrap();
        state.check_and_increment("192.168.1.1").await.unwrap();
        assert!(state.check_and_increment("192.168.1.1").await.is_err());

        // IP 2 should still have its own limit
        assert!(state.check_and_increment("192.168.1.2").await.is_ok());
    }
}
