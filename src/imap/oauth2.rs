// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Microsoft 365 OAuth2 implementation for IMAP
//!
//! Uses the Microsoft Authentication Library (MSAL) compatible device code flow
//! to obtain access tokens for IMAP authentication via XOAUTH2.

use chrono::{DateTime, Utc, Duration};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// OAuth2 errors
#[derive(Error, Debug)]
pub enum OAuth2Error {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Token request failed: {error} - {description}")]
    TokenError { error: String, description: String },

    #[error("Device code expired")]
    DeviceCodeExpired,

    #[error("Authorization pending")]
    AuthorizationPending,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),
}

/// Microsoft OAuth2 configuration
#[derive(Debug, Clone)]
pub struct MicrosoftOAuth2Config {
    pub client_id: String,
    pub tenant_id: String,
    pub scopes: Vec<String>,
}

impl MicrosoftOAuth2Config {
    /// Create config for Microsoft 365 IMAP access
    pub fn for_m365(client_id: impl Into<String>, tenant_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            tenant_id: tenant_id.into(),
            scopes: vec![
                "https://outlook.office365.com/IMAP.AccessAsUser.All".to_string(),
                "offline_access".to_string(),
            ],
        }
    }

    /// Load from environment variables
    pub fn from_env() -> Result<Self, OAuth2Error> {
        let client_id = std::env::var("MICROSOFT_CLIENT_ID")
            .map_err(|_| OAuth2Error::InvalidConfig("MICROSOFT_CLIENT_ID not set".to_string()))?;
        let tenant_id = std::env::var("MICROSOFT_TENANT_ID")
            .unwrap_or_else(|_| "common".to_string());

        Ok(Self::for_m365(client_id, tenant_id))
    }

    fn token_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        )
    }

    fn device_code_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/devicecode",
            self.tenant_id
        )
    }
}

/// OAuth2 token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub token_type: String,
    pub scope: String,
}

/// Token with expiry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub scope: String,
}

impl StoredToken {
    /// Check if token is expired or about to expire (within 5 minutes)
    pub fn is_expired(&self) -> bool {
        Utc::now() + Duration::minutes(5) >= self.expires_at
    }

    /// Create from token response
    pub fn from_response(response: TokenResponse) -> Self {
        let expires_at = Utc::now() + Duration::seconds(response.expires_in);
        Self {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            expires_at,
            scope: response.scope,
        }
    }
}

/// Microsoft OAuth2 client
pub struct MicrosoftOAuth2Client {
    config: MicrosoftOAuth2Config,
    http_client: reqwest::Client,
}

impl MicrosoftOAuth2Client {
    pub fn new(config: MicrosoftOAuth2Config) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, OAuth2Error> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("client_id".to_string(), self.config.client_id.clone());
        params.insert("refresh_token".to_string(), refresh_token.to_string());
        params.insert("grant_type".to_string(), "refresh_token".to_string());
        params.insert("scope".to_string(), self.config.scopes.join(" "));

        let response = self
            .http_client
            .post(&self.config.token_endpoint())
            .form(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let token: TokenResponse = response.json().await?;
            Ok(token)
        } else {
            let error: MicrosoftErrorResponse = response.json().await?;
            Err(OAuth2Error::TokenError {
                error: error.error,
                description: error.error_description,
            })
        }
    }

    /// Get a valid access token, refreshing if necessary
    pub async fn get_valid_token(
        &self,
        stored: &mut StoredToken,
    ) -> Result<String, OAuth2Error> {
        if stored.is_expired() {
            info!("Access token expired, refreshing...");
            if let Some(refresh) = &stored.refresh_token {
                let response = self.refresh_token(refresh).await?;
                *stored = StoredToken::from_response(response);
                info!("Token refreshed successfully");
            } else {
                return Err(OAuth2Error::RefreshFailed(
                    "No refresh token available".to_string(),
                ));
            }
        }
        Ok(stored.access_token.clone())
    }
}

/// Microsoft error response
#[derive(Debug, Deserialize)]
struct MicrosoftErrorResponse {
    error: String,
    error_description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_token_expiry() {
        let token = StoredToken {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + Duration::minutes(10),
            scope: "test".to_string(),
        };
        assert!(!token.is_expired());

        let expired = StoredToken {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() - Duration::minutes(1),
            scope: "test".to_string(),
        };
        assert!(expired.is_expired());
    }
}
