//! Request validation module for REST API
//!
//! This module provides comprehensive validation for all API request payloads,
//! ensuring data integrity and preventing invalid operations.

use actix_web::web::Json;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::{Validate, ValidationError};

use crate::api::errors::ApiError;

/// Email validation regex pattern
const EMAIL_REGEX: &str = r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$";

/// Folder name validation regex (no special IMAP characters)
const FOLDER_NAME_REGEX: &str = r"^[a-zA-Z0-9_\-\.\s]+$";

/// Maximum folder name length
const MAX_FOLDER_NAME_LENGTH: usize = 255;

/// Maximum email size in bytes (25MB)
const MAX_EMAIL_SIZE: usize = 25 * 1024 * 1024;

/// Custom validation functions
pub mod validators {
    use super::*;

    /// Validate email address format
    pub fn validate_email(email: &str) -> Result<(), ValidationError> {
        let email_regex = Regex::new(EMAIL_REGEX).unwrap();
        if !email_regex.is_match(email) {
            return Err(ValidationError::new("invalid_email_format"));
        }
        Ok(())
    }

    /// Validate folder name
    pub fn validate_folder_name(name: &str) -> Result<(), ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::new("folder_name_empty"));
        }

        if name.len() > MAX_FOLDER_NAME_LENGTH {
            return Err(ValidationError::new("folder_name_too_long"));
        }

        let folder_regex = Regex::new(FOLDER_NAME_REGEX).unwrap();
        if !folder_regex.is_match(name) {
            return Err(ValidationError::new("invalid_folder_name_characters"));
        }

        // Check for reserved IMAP folder names
        let reserved = ["INBOX", "Trash", "Sent", "Drafts", "Spam", "Junk"];
        if reserved.iter().any(|&r| r.eq_ignore_ascii_case(name)) {
            return Err(ValidationError::new("reserved_folder_name"));
        }

        Ok(())
    }

    /// Validate base64 content
    pub fn validate_base64(content: &str) -> Result<(), ValidationError> {
        use base64::Engine;

        if content.is_empty() {
            return Err(ValidationError::new("empty_content"));
        }

        // Try to decode to validate format
        match base64::engine::general_purpose::STANDARD.decode(content) {
            Ok(decoded) => {
                if decoded.len() > MAX_EMAIL_SIZE {
                    return Err(ValidationError::new("content_too_large"));
                }
                Ok(())
            },
            Err(_) => Err(ValidationError::new("invalid_base64")),
        }
    }

    /// Validate UIDs list
    pub fn validate_uids(uids: &[u32]) -> Result<(), ValidationError> {
        if uids.is_empty() {
            return Err(ValidationError::new("uids_empty"));
        }

        if uids.len() > 1000 {
            return Err(ValidationError::new("too_many_uids"));
        }

        // Check for valid UID values (> 0)
        if uids.iter().any(|&uid| uid == 0) {
            return Err(ValidationError::new("invalid_uid_value"));
        }

        Ok(())
    }

    /// Validate search query
    pub fn validate_search_query(query: &str) -> Result<(), ValidationError> {
        if query.len() > 1000 {
            return Err(ValidationError::new("search_query_too_long"));
        }

        // Basic IMAP search command validation
        let valid_commands = [
            "ALL", "ANSWERED", "DELETED", "DRAFT", "FLAGGED", "NEW", "OLD",
            "RECENT", "SEEN", "UNANSWERED", "UNDELETED", "UNDRAFT", "UNFLAGGED",
            "UNSEEN", "FROM", "TO", "CC", "BCC", "SUBJECT", "BODY", "TEXT",
            "KEYWORD", "BEFORE", "ON", "SINCE", "SENTBEFORE", "SENTON", "SENTSINCE",
            "SMALLER", "LARGER", "UID", "OR", "NOT"
        ];

        let query_upper = query.to_uppercase();
        let has_valid_command = valid_commands.iter()
            .any(|&cmd| query_upper.contains(cmd));

        if !has_valid_command && query != "*" {
            return Err(ValidationError::new("invalid_search_command"));
        }

        Ok(())
    }

    /// Validate pagination parameters
    pub fn validate_pagination(limit: Option<usize>, offset: Option<usize>) -> Result<(usize, usize), ValidationError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        if limit == 0 || limit > 100 {
            return Err(ValidationError::new("invalid_limit"));
        }

        if offset > 10000 {
            return Err(ValidationError::new("offset_too_large"));
        }

        Ok((limit, offset))
    }
}

// === Validated Request Structures ===

/// Validated folder creation request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedCreateFolderRequest {
    #[validate(custom(function = "validators::validate_folder_name"))]
    pub name: String,
    pub parent: Option<String>,
}

/// Validated folder update request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedUpdateFolderRequest {
    pub name: Option<String>,
}

/// Validated email creation request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedCreateEmailRequest {
    #[validate(custom(function = "validators::validate_base64"))]
    pub content: String,
    pub flags: Option<Vec<String>>,
}

/// Validated email search request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedSearchRequest {
    #[validate(custom(function = "validators::validate_search_query"))]
    pub query: String,
    pub folder: Option<String>,
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<usize>,
    #[validate(range(min = 0, max = 10000))]
    pub offset: Option<usize>,
}

/// Validated move email request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedMoveEmailRequest {
    #[validate(custom(function = "validators::validate_folder_name"))]
    pub to_folder: String,
}

/// Validated API key creation request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedCreateApiKeyRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    pub imap_credentials: ValidatedImapCredentials,
    pub scopes: Option<Vec<String>>,
}

/// Validated IMAP credentials
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ValidatedImapCredentials {
    #[validate(length(min = 1, max = 255))]
    pub username: String,
    #[validate(length(min = 1, max = 255))]
    pub password: String,
    #[validate(length(min = 1, max = 255))]
    pub server: String,
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,
}

// === Validation Middleware ===

/// Validate request payload using the Validate trait
pub async fn validate_payload<T>(payload: Json<T>) -> Result<T, ApiError>
where
    T: Validate,
{
    let inner = payload.into_inner();
    match inner.validate() {
        Ok(_) => Ok(inner),
        Err(errors) => Err(errors.into()),
    }
}

/// Validate path parameters
pub fn validate_path_param(param: &str, param_name: &str) -> Result<(), ApiError> {
    if param.is_empty() {
        return Err(ApiError::InvalidPathParam {
            param: param_name.to_string(),
            reason: "Cannot be empty".to_string(),
        });
    }

    // Check for path traversal attempts
    if param.contains("..") || param.contains("/") || param.contains("\\") {
        return Err(ApiError::InvalidPathParam {
            param: param_name.to_string(),
            reason: "Contains invalid characters (path traversal attempt detected)".to_string(),
        });
    }

    Ok(())
}

/// Validate query parameters
pub fn validate_query_params(query: &HashMap<String, String>) -> Result<(), ApiError> {
    // Check for SQL injection patterns
    let dangerous_patterns = ["';", "--", "/*", "*/", "xp_", "sp_", "exec", "execute"];

    for (key, value) in query.iter() {
        let lower_value = value.to_lowercase();
        if dangerous_patterns.iter().any(|&pattern| lower_value.contains(pattern)) {
            return Err(ApiError::InvalidQueryParam {
                param: key.clone(),
                reason: "Contains potentially dangerous patterns".to_string(),
            });
        }
    }

    Ok(())
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute per API key
    pub per_key_per_minute: u32,
    /// Requests per hour per API key
    pub per_key_per_hour: u32,
    /// Requests per minute per IP
    pub per_ip_per_minute: u32,
    /// Requests per hour per IP
    pub per_ip_per_hour: u32,
    /// Global requests per minute
    pub global_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            per_key_per_minute: 60,
            per_key_per_hour: 1000,
            per_ip_per_minute: 30,
            per_ip_per_hour: 500,
            global_per_minute: 1000,
        }
    }
}

/// Enhanced rate limiter with IP-based limiting
pub struct EnhancedRateLimiter {
    config: RateLimitConfig,
    /// API key counters (handled by ApiKeyStore)
    /// IP address counters
    ip_counters: std::sync::Arc<tokio::sync::RwLock<HashMap<String, RequestCounter>>>,
    /// Global counter
    global_counter: std::sync::Arc<tokio::sync::RwLock<RequestCounter>>,
}

#[derive(Debug, Clone)]
struct RequestCounter {
    minute_count: u32,
    minute_reset: chrono::DateTime<chrono::Utc>,
    hour_count: u32,
    hour_reset: chrono::DateTime<chrono::Utc>,
}

impl EnhancedRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_counters: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            global_counter: std::sync::Arc::new(tokio::sync::RwLock::new(RequestCounter {
                minute_count: 0,
                minute_reset: chrono::Utc::now() + chrono::Duration::minutes(1),
                hour_count: 0,
                hour_reset: chrono::Utc::now() + chrono::Duration::hours(1),
            })),
        }
    }

    /// Check IP-based rate limits
    pub async fn check_ip_limit(&self, ip: &str) -> Result<(), ApiError> {
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

        // Reset counters if needed
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
            return Err(ApiError::RateLimitExceeded {
                message: format!("IP rate limit exceeded: {} requests per minute", self.config.per_ip_per_minute),
            });
        }
        if counter.hour_count >= self.config.per_ip_per_hour {
            return Err(ApiError::RateLimitExceeded {
                message: format!("IP rate limit exceeded: {} requests per hour", self.config.per_ip_per_hour),
            });
        }

        // Increment
        counter.minute_count += 1;
        counter.hour_count += 1;

        Ok(())
    }

    /// Check global rate limits
    pub async fn check_global_limit(&self) -> Result<(), ApiError> {
        let mut counter = self.global_counter.write().await;
        let now = chrono::Utc::now();

        // Reset if needed
        if now > counter.minute_reset {
            counter.minute_count = 0;
            counter.minute_reset = now + chrono::Duration::minutes(1);
        }

        // Check limit
        if counter.minute_count >= self.config.global_per_minute {
            return Err(ApiError::RateLimitExceeded {
                message: format!("Global rate limit exceeded: {} requests per minute", self.config.global_per_minute),
            });
        }

        // Increment
        counter.minute_count += 1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        assert!(validators::validate_email("test@example.com").is_ok());
        assert!(validators::validate_email("invalid.email").is_err());
        assert!(validators::validate_email("@example.com").is_err());
        assert!(validators::validate_email("test@").is_err());
    }

    #[test]
    fn test_folder_name_validation() {
        assert!(validators::validate_folder_name("MyFolder").is_ok());
        assert!(validators::validate_folder_name("My_Folder-2023").is_ok());
        assert!(validators::validate_folder_name("").is_err());
        assert!(validators::validate_folder_name("Folder/Name").is_err());
        assert!(validators::validate_folder_name("INBOX").is_err()); // Reserved
    }

    #[test]
    fn test_base64_validation() {
        let valid_base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            b"Hello, World!"
        );
        assert!(validators::validate_base64(&valid_base64).is_ok());
        assert!(validators::validate_base64("not-base64!@#").is_err());
        assert!(validators::validate_base64("").is_err());
    }

    #[test]
    fn test_uids_validation() {
        assert!(validators::validate_uids(&[1, 2, 3]).is_ok());
        assert!(validators::validate_uids(&[]).is_err());
        assert!(validators::validate_uids(&[0, 1, 2]).is_err()); // Contains 0

        let too_many: Vec<u32> = (1..=1001).collect();
        assert!(validators::validate_uids(&too_many).is_err());
    }

    #[test]
    fn test_search_query_validation() {
        assert!(validators::validate_search_query("FROM john@example.com").is_ok());
        assert!(validators::validate_search_query("SUBJECT meeting").is_ok());
        assert!(validators::validate_search_query("ALL").is_ok());
        assert!(validators::validate_search_query("*").is_ok());

        let long_query = "x".repeat(1001);
        assert!(validators::validate_search_query(&long_query).is_err());

        assert!(validators::validate_search_query("xyz abc").is_err());
    }

    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let config = RateLimitConfig {
            per_ip_per_minute: 2,
            per_ip_per_hour: 10,
            ..Default::default()
        };

        let limiter = EnhancedRateLimiter::new(config);

        // First two requests should succeed
        assert!(limiter.check_ip_limit("192.168.1.1").await.is_ok());
        assert!(limiter.check_ip_limit("192.168.1.1").await.is_ok());

        // Third request should fail
        assert!(limiter.check_ip_limit("192.168.1.1").await.is_err());

        // Different IP should work
        assert!(limiter.check_ip_limit("192.168.1.2").await.is_ok());
    }
}