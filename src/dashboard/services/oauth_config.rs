// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! OAuth2 configuration for Microsoft 365 and other providers.
//!
//! Loads client credentials from environment variables.
//! Microsoft endpoints are constants (they don't change).

use log::{info, debug};
use serde::{Deserialize, Serialize};

/// Microsoft OAuth2 authorization endpoint (tenant-specific or "common").
pub fn microsoft_auth_url() -> String {
    let tenant = std::env::var("MICROSOFT_TENANT_ID").unwrap_or_else(|_| "common".to_string());
    format!("https://login.microsoftonline.com/{}/oauth2/v2.0/authorize", tenant)
}

/// Microsoft OAuth2 token endpoint (tenant-specific or "common").
pub fn microsoft_token_url() -> String {
    let tenant = std::env::var("MICROSOFT_TENANT_ID").unwrap_or_else(|_| "common".to_string());
    format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant)
}

/// Required scopes for IMAP + SMTP access via OAuth2.
pub const MICROSOFT_SCOPES: &[&str] = &[
    "https://outlook.office365.com/IMAP.AccessAsUser.All",
    "https://outlook.office365.com/SMTP.Send",
    "offline_access",
];

/// OAuth2 provider configuration loaded from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    /// Azure AD application client ID.
    pub client_id: String,
    /// Azure AD application client secret.
    pub client_secret: String,
    /// Base URL for OAuth callback redirects (e.g., "http://localhost:9439").
    pub redirect_base_url: String,
}

impl OAuthProviderConfig {
    /// Build the full redirect URI for the Microsoft OAuth callback.
    pub fn microsoft_redirect_uri(&self) -> String {
        format!(
            "{}/api/dashboard/oauth/callback/microsoft",
            self.redirect_base_url.trim_end_matches('/')
        )
    }
}

/// Top-level OAuth configuration that holds per-provider configs.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// Microsoft 365 OAuth credentials (None if not configured).
    pub microsoft: Option<OAuthProviderConfig>,
}

impl OAuthConfig {
    /// Load OAuth configuration from environment variables.
    ///
    /// Returns an `OAuthConfig` with `microsoft = Some(...)` only when
    /// all three env vars (`MICROSOFT_CLIENT_ID`, `MICROSOFT_CLIENT_SECRET`,
    /// `OAUTH_REDIRECT_BASE_URL`) are set and non-empty.
    pub fn from_env() -> Self {
        let microsoft = Self::load_microsoft_config();

        if microsoft.is_some() {
            info!("Microsoft OAuth2 configuration loaded from environment");
        } else {
            debug!("Microsoft OAuth2 not configured (set MICROSOFT_CLIENT_ID, MICROSOFT_CLIENT_SECRET, OAUTH_REDIRECT_BASE_URL)");
        }

        Self { microsoft }
    }

    /// Returns true if at least one OAuth provider is configured.
    pub fn has_any_provider(&self) -> bool {
        self.microsoft.is_some()
    }

    /// Returns true if Microsoft OAuth is configured.
    pub fn has_microsoft(&self) -> bool {
        self.microsoft.is_some()
    }

    fn load_microsoft_config() -> Option<OAuthProviderConfig> {
        let client_id = std::env::var("MICROSOFT_CLIENT_ID").ok()?;
        let client_secret = std::env::var("MICROSOFT_CLIENT_SECRET").ok()?;
        let redirect_base_url = std::env::var("OAUTH_REDIRECT_BASE_URL").ok()?;

        // All three must be non-empty.
        if client_id.is_empty() || client_secret.is_empty() || redirect_base_url.is_empty() {
            return None;
        }

        Some(OAuthProviderConfig {
            client_id,
            client_secret,
            redirect_base_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_oauth_config_loads_when_all_vars_set() {
        std::env::set_var("MICROSOFT_CLIENT_ID", "test-client-id");
        std::env::set_var("MICROSOFT_CLIENT_SECRET", "test-client-secret");
        std::env::set_var("OAUTH_REDIRECT_BASE_URL", "http://localhost:9439");

        let config = OAuthConfig::from_env();
        assert!(config.has_microsoft());
        assert!(config.has_any_provider());

        let ms = config.microsoft.unwrap();
        assert_eq!(ms.client_id, "test-client-id");
        assert_eq!(ms.client_secret, "test-client-secret");
        assert_eq!(ms.redirect_base_url, "http://localhost:9439");

        std::env::remove_var("MICROSOFT_CLIENT_ID");
        std::env::remove_var("MICROSOFT_CLIENT_SECRET");
        std::env::remove_var("OAUTH_REDIRECT_BASE_URL");
    }

    #[test]
    #[serial]
    fn test_oauth_config_none_when_vars_missing() {
        std::env::remove_var("MICROSOFT_CLIENT_ID");
        std::env::remove_var("MICROSOFT_CLIENT_SECRET");
        std::env::remove_var("OAUTH_REDIRECT_BASE_URL");

        let config = OAuthConfig::from_env();
        assert!(!config.has_microsoft());
        assert!(!config.has_any_provider());
    }

    #[test]
    #[serial]
    fn test_oauth_config_none_when_vars_empty() {
        std::env::set_var("MICROSOFT_CLIENT_ID", "");
        std::env::set_var("MICROSOFT_CLIENT_SECRET", "test-secret");
        std::env::set_var("OAUTH_REDIRECT_BASE_URL", "http://localhost:9439");

        let config = OAuthConfig::from_env();
        assert!(!config.has_microsoft());

        std::env::remove_var("MICROSOFT_CLIENT_ID");
        std::env::remove_var("MICROSOFT_CLIENT_SECRET");
        std::env::remove_var("OAUTH_REDIRECT_BASE_URL");
    }

    #[test]
    fn test_microsoft_redirect_uri() {
        let provider = OAuthProviderConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            redirect_base_url: "http://localhost:9439".to_string(),
        };
        assert_eq!(
            provider.microsoft_redirect_uri(),
            "http://localhost:9439/api/dashboard/oauth/callback/microsoft"
        );
    }

    #[test]
    fn test_microsoft_redirect_uri_strips_trailing_slash() {
        let provider = OAuthProviderConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            redirect_base_url: "http://localhost:9439/".to_string(),
        };
        assert_eq!(
            provider.microsoft_redirect_uri(),
            "http://localhost:9439/api/dashboard/oauth/callback/microsoft"
        );
    }

    #[test]
    fn test_microsoft_endpoints() {
        assert!(microsoft_auth_url().contains("login.microsoftonline.com"));
        assert!(microsoft_token_url().contains("login.microsoftonline.com"));
        assert_eq!(MICROSOFT_SCOPES.len(), 3);
        assert!(MICROSOFT_SCOPES.contains(&"offline_access"));
    }
}
