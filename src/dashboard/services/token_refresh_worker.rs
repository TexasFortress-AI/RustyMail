// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Background worker that automatically refreshes expiring OAuth tokens.
//!
//! Periodically checks all OAuth-configured accounts and refreshes their
//! access tokens before they expire, preventing silent IMAP auth failures.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::Mutex as TokioMutex;
use log::{info, error, warn, debug};
use crate::dashboard::services::{AccountService, OAuthService, OAuthTokens};

/// Default interval between token refresh checks (seconds).
const DEFAULT_CHECK_INTERVAL_SECONDS: u64 = 300;

/// Default margin before expiry to trigger a refresh (seconds).
/// Tokens expiring within this window will be proactively refreshed.
const DEFAULT_REFRESH_MARGIN_SECONDS: i64 = 3600;

/// Background worker that checks for expiring OAuth tokens and refreshes them.
pub struct TokenRefreshWorker {
    account_service: Arc<TokioMutex<AccountService>>,
    oauth_service: Arc<OAuthService>,
    poll_interval: Duration,
    refresh_margin_seconds: i64,
}

impl TokenRefreshWorker {
    /// Create a new TokenRefreshWorker.
    ///
    /// Reads `TOKEN_REFRESH_CHECK_INTERVAL_SECONDS` from the environment
    /// (default: 300 seconds = 5 minutes).
    pub fn new(
        account_service: Arc<TokioMutex<AccountService>>,
        oauth_service: Arc<OAuthService>,
    ) -> Self {
        let poll_interval = std::env::var("TOKEN_REFRESH_CHECK_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_CHECK_INTERVAL_SECONDS);

        Self {
            account_service,
            oauth_service,
            poll_interval: Duration::from_secs(poll_interval),
            refresh_margin_seconds: DEFAULT_REFRESH_MARGIN_SECONDS,
        }
    }

    /// Start the background worker loop. Runs indefinitely.
    pub async fn start(self: Arc<Self>) {
        info!(
            "Starting token refresh worker with {} second poll interval (refresh margin: {}s)",
            self.poll_interval.as_secs(),
            self.refresh_margin_seconds,
        );

        loop {
            self.check_and_refresh_all().await;
            sleep(self.poll_interval).await;
        }
    }

    /// Check all OAuth accounts and refresh tokens that are expiring soon.
    async fn check_and_refresh_all(&self) {
        // Lock account_service briefly to get the account list, then release
        let accounts = {
            let service = self.account_service.lock().await;
            match service.list_accounts().await {
                Ok(accts) => accts,
                Err(e) => {
                    error!("Token refresh: failed to list accounts: {}", e);
                    return;
                }
            }
        };

        let oauth_accounts: Vec<_> = accounts
            .into_iter()
            .filter(|a| a.oauth_provider.is_some())
            .collect();

        if oauth_accounts.is_empty() {
            debug!("Token refresh: no OAuth accounts configured, skipping");
            return;
        }

        debug!("Token refresh: checking {} OAuth account(s)", oauth_accounts.len());

        for account in oauth_accounts {
            let email = &account.email_address;

            // Skip accounts without a refresh token
            let refresh_token = match &account.oauth_refresh_token {
                Some(rt) if !rt.is_empty() => rt.clone(),
                _ => {
                    debug!("Token refresh: skipping {} (no refresh token)", email);
                    continue;
                }
            };

            // Build OAuthTokens to check expiry
            let tokens = OAuthTokens {
                access_token: account.oauth_access_token.clone().unwrap_or_default(),
                refresh_token: refresh_token.clone(),
                expires_at: account.oauth_token_expiry.unwrap_or(0),
                email: email.clone(),
            };

            if !tokens.is_expired(self.refresh_margin_seconds) {
                debug!(
                    "Token refresh: {} token still valid (expires_at={}, margin={}s)",
                    email, tokens.expires_at, self.refresh_margin_seconds,
                );
                continue;
            }

            info!("Token refresh: refreshing expiring token for {}", email);

            match self.oauth_service.refresh_token(&refresh_token).await {
                Ok(token_response) => {
                    let new_expires_at =
                        chrono::Utc::now().timestamp() + token_response.expires_in as i64;

                    let service = self.account_service.lock().await;
                    if let Err(e) = service
                        .update_oauth_tokens(
                            email,
                            &token_response.access_token,
                            token_response.refresh_token.as_deref(),
                            new_expires_at,
                        )
                        .await
                    {
                        error!("Token refresh: failed to persist new tokens for {}: {}", email, e);
                    } else {
                        info!(
                            "Token refresh: successfully refreshed token for {} (new expiry: {})",
                            email, new_expires_at,
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Token refresh: failed to refresh token for {} (will retry next cycle): {}",
                        email, e,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_needs_refresh_when_expired() {
        let now = chrono::Utc::now().timestamp();
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: now - 60, // Already expired
            email: "user@test.com".to_string(),
        };
        assert!(tokens.is_expired(0));
        assert!(tokens.is_expired(DEFAULT_REFRESH_MARGIN_SECONDS));
    }

    #[test]
    fn test_token_needs_refresh_within_margin() {
        let now = chrono::Utc::now().timestamp();
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: now + 1800, // 30 min left
            email: "user@test.com".to_string(),
        };
        // 1-hour margin: should need refresh (30 min < 60 min margin)
        assert!(tokens.is_expired(DEFAULT_REFRESH_MARGIN_SECONDS));
        // 0 margin: still valid
        assert!(!tokens.is_expired(0));
    }

    #[test]
    fn test_token_valid_outside_margin() {
        let now = chrono::Utc::now().timestamp();
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: now + 7200, // 2 hours left
            email: "user@test.com".to_string(),
        };
        // 1-hour margin: should NOT need refresh (2h > 1h margin)
        assert!(!tokens.is_expired(DEFAULT_REFRESH_MARGIN_SECONDS));
    }

    #[test]
    fn test_token_with_zero_expiry_always_expired() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: 0, // Missing/zero expiry
            email: "user@test.com".to_string(),
        };
        assert!(tokens.is_expired(DEFAULT_REFRESH_MARGIN_SECONDS));
    }
}
