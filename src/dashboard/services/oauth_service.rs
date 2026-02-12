// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! OAuth2 service for Microsoft 365 authorization code flow with PKCE.
//!
//! Handles:
//! - Generating authorization URLs with PKCE + state
//! - Exchanging authorization codes for tokens
//! - Refreshing expired access tokens

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL};
use log::{debug, error, info};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

use super::oauth_config::{
    OAuthConfig, OAuthProviderConfig, microsoft_auth_url, microsoft_token_url, MICROSOFT_SCOPES,
};
use super::encryption::CredentialEncryption;

/// Errors from OAuth2 operations.
#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Microsoft OAuth is not configured")]
    NotConfigured,
    #[error("Invalid state parameter (possible CSRF)")]
    InvalidState,
    #[error("No pending authorization for state: {0}")]
    NoPendingAuth(String),
    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),
    #[error("Token refresh failed: {0}")]
    TokenRefreshFailed(String),
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Encryption error: {0}")]
    EncryptionError(#[from] super::encryption::EncryptionError),
}

/// Token response from Microsoft OAuth2 token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// Stored OAuth tokens for an account (plaintext, encryption done at storage layer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp (seconds) when the access token expires.
    pub expires_at: i64,
    pub email: String,
}

impl OAuthTokens {
    /// Returns true if the access token is expired or will expire within `margin_seconds`.
    pub fn is_expired(&self, margin_seconds: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at - margin_seconds <= now
    }
}

/// Pending authorization data stored between the authorize redirect and callback.
#[derive(Debug, Clone)]
struct PendingAuth {
    code_verifier: String,
    provider: String,
}

/// OAuth2 service managing the authorization code flow with PKCE.
pub struct OAuthService {
    config: OAuthConfig,
    http_client: reqwest::Client,
    /// Map from state parameter → pending auth data (in-memory, short-lived).
    pending_auths: Arc<Mutex<HashMap<String, PendingAuth>>>,
}

impl OAuthService {
    /// Create a new OAuthService from the given config.
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            pending_auths: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns true if Microsoft OAuth is configured.
    pub fn is_microsoft_configured(&self) -> bool {
        self.config.has_microsoft()
    }

    /// Returns the OAuth redirect base URL (e.g., "http://localhost:9439").
    /// Used by the callback handler to redirect back to the frontend after OAuth.
    pub fn redirect_base_url(&self) -> Option<&str> {
        self.config.microsoft.as_ref().map(|c| c.redirect_base_url.as_str())
    }

    /// Generate a Microsoft OAuth2 authorization URL with PKCE.
    ///
    /// Returns `(authorization_url, state)`. The state is used to correlate
    /// the callback with this request.
    pub async fn generate_microsoft_auth_url(&self) -> Result<(String, String), OAuthError> {
        let ms_config = self.config.microsoft.as_ref()
            .ok_or(OAuthError::NotConfigured)?;

        let state = generate_random_string(32);
        let code_verifier = generate_code_verifier();
        let code_challenge = compute_code_challenge(&code_verifier);

        // Store pending auth for callback
        {
            let mut pending = self.pending_auths.lock().await;
            pending.insert(state.clone(), PendingAuth {
                code_verifier,
                provider: "microsoft".to_string(),
            });
        }

        let scopes = MICROSOFT_SCOPES.join(" ");
        let redirect_uri = ms_config.microsoft_redirect_uri();

        let auth_url = format!(
            "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&response_mode=query",
            microsoft_auth_url(),
            urlencoding::encode(&ms_config.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&scopes),
            urlencoding::encode(&state),
            urlencoding::encode(&code_challenge),
        );

        debug!("Generated Microsoft OAuth2 authorization URL (state={})", &state[..8]);
        Ok((auth_url, state))
    }

    /// Exchange an authorization code for tokens.
    ///
    /// `state` and `code` come from the OAuth callback query parameters.
    pub async fn exchange_code(
        &self,
        state: &str,
        code: &str,
    ) -> Result<OAuthTokenResponse, OAuthError> {
        let ms_config = self.config.microsoft.as_ref()
            .ok_or(OAuthError::NotConfigured)?;

        // Retrieve and remove the pending auth
        let pending = {
            let mut pending_map = self.pending_auths.lock().await;
            pending_map.remove(state)
                .ok_or_else(|| OAuthError::NoPendingAuth(state.to_string()))?
        };

        let redirect_uri = ms_config.microsoft_redirect_uri();

        let params = [
            ("client_id", ms_config.client_id.as_str()),
            ("client_secret", ms_config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", pending.code_verifier.as_str()),
        ];

        info!("Exchanging authorization code for tokens (provider={})", pending.provider);

        let response = self.http_client
            .post(&microsoft_token_url())
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Token exchange failed: HTTP {} - {}", status, body);
            return Err(OAuthError::TokenExchangeFailed(
                format!("HTTP {}: {}", status, body)
            ));
        }

        let token_response: OAuthTokenResponse = response.json().await
            .map_err(|e| OAuthError::TokenExchangeFailed(format!("JSON parse: {}", e)))?;

        info!("Successfully exchanged authorization code for tokens");
        Ok(token_response)
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse, OAuthError> {
        let ms_config = self.config.microsoft.as_ref()
            .ok_or(OAuthError::NotConfigured)?;

        let scopes = MICROSOFT_SCOPES.join(" ");

        let params = [
            ("client_id", ms_config.client_id.as_str()),
            ("client_secret", ms_config.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
            ("scope", scopes.as_str()),
        ];

        debug!("Refreshing Microsoft OAuth2 access token");

        let response = self.http_client
            .post(&microsoft_token_url())
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Token refresh failed: HTTP {} - {}", status, body);
            return Err(OAuthError::TokenRefreshFailed(
                format!("HTTP {}: {}", status, body)
            ));
        }

        let token_response: OAuthTokenResponse = response.json().await
            .map_err(|e| OAuthError::TokenRefreshFailed(format!("JSON parse: {}", e)))?;

        info!("Successfully refreshed Microsoft OAuth2 access token");
        Ok(token_response)
    }

    /// Build the XOAUTH2 token string for IMAP/SMTP authentication.
    ///
    /// Format: `user=<email>\x01auth=Bearer <access_token>\x01\x01`
    pub fn build_xoauth2_token(email: &str, access_token: &str) -> String {
        format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token)
    }
}

/// Generate a cryptographically random URL-safe string of the given byte length.
fn generate_random_string(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    BASE64URL.encode(&bytes)
}

/// Generate an OAuth2 PKCE code verifier (43-128 character URL-safe string).
fn generate_code_verifier() -> String {
    generate_random_string(32) // 32 bytes → 43 base64url characters
}

/// Compute the S256 code challenge from a code verifier.
fn compute_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    BASE64URL.encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_string_length() {
        let s = generate_random_string(32);
        // 32 bytes base64url encoded = 43 chars
        assert_eq!(s.len(), 43);
    }

    #[test]
    fn test_generate_random_string_uniqueness() {
        let s1 = generate_random_string(32);
        let s2 = generate_random_string(32);
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_code_verifier_length() {
        let verifier = generate_code_verifier();
        // Must be 43-128 chars per RFC 7636
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
    }

    #[test]
    fn test_code_challenge_is_base64url() {
        let verifier = generate_code_verifier();
        let challenge = compute_code_challenge(&verifier);
        // SHA256 = 32 bytes → 43 base64url chars
        assert_eq!(challenge.len(), 43);
        // Should only contain URL-safe base64 chars
        assert!(challenge.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_code_challenge_deterministic() {
        let verifier = "test-verifier-string";
        let c1 = compute_code_challenge(verifier);
        let c2 = compute_code_challenge(verifier);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_code_challenge_different_for_different_verifiers() {
        let c1 = compute_code_challenge("verifier-1");
        let c2 = compute_code_challenge("verifier-2");
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_build_xoauth2_token() {
        let token = OAuthService::build_xoauth2_token("user@outlook.com", "my-access-token");
        assert_eq!(token, "user=user@outlook.com\x01auth=Bearer my-access-token\x01\x01");
    }

    #[test]
    fn test_oauth_tokens_is_expired() {
        let now = chrono::Utc::now().timestamp();

        // Token that expires in 10 minutes — not expired
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now + 600,
            email: "user@test.com".to_string(),
        };
        assert!(!tokens.is_expired(300)); // 5-min margin → still valid

        // Token that expires in 2 minutes — expired with 5-min margin
        let tokens_soon = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now + 120,
            email: "user@test.com".to_string(),
        };
        assert!(tokens_soon.is_expired(300)); // 5-min margin → considered expired

        // Token already expired
        let tokens_past = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now - 60,
            email: "user@test.com".to_string(),
        };
        assert!(tokens_past.is_expired(0));
    }

    #[tokio::test]
    async fn test_oauth_service_not_configured() {
        let config = OAuthConfig { microsoft: None };
        let service = OAuthService::new(config);

        assert!(!service.is_microsoft_configured());

        let result = service.generate_microsoft_auth_url().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OAuthError::NotConfigured));
    }

    #[tokio::test]
    async fn test_generate_auth_url_format() {
        let config = OAuthConfig {
            microsoft: Some(OAuthProviderConfig {
                client_id: "test-client-id".to_string(),
                client_secret: "test-secret".to_string(),
                redirect_base_url: "http://localhost:9439".to_string(),
            }),
        };
        let service = OAuthService::new(config);

        let (url, state) = service.generate_microsoft_auth_url().await.unwrap();

        assert!(url.starts_with(&microsoft_auth_url()));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(urlencoding::encode(&state).as_ref()));
        assert!(url.contains("IMAP.AccessAsUser.All"));
        assert!(url.contains("SMTP.Send"));
        assert!(url.contains("offline_access"));
    }

    #[tokio::test]
    async fn test_exchange_code_invalid_state() {
        let config = OAuthConfig {
            microsoft: Some(OAuthProviderConfig {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                redirect_base_url: "http://localhost:9439".to_string(),
            }),
        };
        let service = OAuthService::new(config);

        let result = service.exchange_code("nonexistent-state", "some-code").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OAuthError::NoPendingAuth(_)));
    }

    #[tokio::test]
    async fn test_pending_auth_consumed_on_exchange() {
        let config = OAuthConfig {
            microsoft: Some(OAuthProviderConfig {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                redirect_base_url: "http://localhost:9439".to_string(),
            }),
        };
        let service = OAuthService::new(config);

        // Generate URL stores pending auth
        let (_url, state) = service.generate_microsoft_auth_url().await.unwrap();

        // Verify it's stored
        {
            let pending = service.pending_auths.lock().await;
            assert!(pending.contains_key(&state));
        }

        // Exchange will fail (no real server) but should consume the pending auth
        let _result = service.exchange_code(&state, "fake-code").await;

        // Pending auth should be consumed
        {
            let pending = service.pending_auths.lock().await;
            assert!(!pending.contains_key(&state));
        }
    }
}
