// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Comprehensive unit tests for Microsoft 365 OAuth2 support.

use rustymail::dashboard::services::oauth_config::*;
use rustymail::dashboard::services::oauth_service::*;
use rustymail::dashboard::services::account_store::*;
use rustymail::imap::xoauth2::XOAuth2Authenticator;
use async_imap::Authenticator;
use serial_test::serial;
use tempfile::TempDir;
use chrono::Utc;

// ============================================================================
// OAuth Config Tests
// ============================================================================

#[test]
#[serial]
fn test_oauth_config_from_env_complete() {
    std::env::set_var("MICROSOFT_CLIENT_ID", "app-client-id-123");
    std::env::set_var("MICROSOFT_CLIENT_SECRET", "super-secret-value");
    std::env::set_var("OAUTH_REDIRECT_BASE_URL", "http://localhost:9439");

    let config = OAuthConfig::from_env();
    assert!(config.has_microsoft());
    assert!(config.has_any_provider());

    let ms = config.microsoft.as_ref().unwrap();
    assert_eq!(ms.client_id, "app-client-id-123");
    assert_eq!(ms.client_secret, "super-secret-value");
    assert_eq!(ms.redirect_base_url, "http://localhost:9439");

    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("MICROSOFT_CLIENT_SECRET");
    std::env::remove_var("OAUTH_REDIRECT_BASE_URL");
}

#[test]
#[serial]
fn test_oauth_config_partial_env_returns_none() {
    // Only one of three vars set
    std::env::set_var("MICROSOFT_CLIENT_ID", "some-id");
    std::env::remove_var("MICROSOFT_CLIENT_SECRET");
    std::env::remove_var("OAUTH_REDIRECT_BASE_URL");

    let config = OAuthConfig::from_env();
    assert!(!config.has_microsoft());

    std::env::remove_var("MICROSOFT_CLIENT_ID");
}

#[test]
fn test_microsoft_redirect_uri_construction() {
    let provider = OAuthProviderConfig {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        redirect_base_url: "https://app.example.com".to_string(),
    };
    assert_eq!(
        provider.microsoft_redirect_uri(),
        "https://app.example.com/api/dashboard/oauth/callback/microsoft"
    );
}

#[test]
fn test_microsoft_endpoints_valid() {
    assert!(microsoft_auth_url().starts_with("https://"));
    assert!(microsoft_token_url().starts_with("https://"));
    assert!(MICROSOFT_SCOPES.contains(&"offline_access"));
    assert!(MICROSOFT_SCOPES.iter().any(|s| s.contains("IMAP")));
    assert!(MICROSOFT_SCOPES.iter().any(|s| s.contains("SMTP")));
}

// ============================================================================
// OAuth Service Tests
// ============================================================================

#[tokio::test]
async fn test_oauth_service_unconfigured_rejects_auth_url() {
    let config = OAuthConfig { microsoft: None };
    let service = OAuthService::new(config);
    assert!(!service.is_microsoft_configured());

    let result = service.generate_microsoft_auth_url().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_oauth_service_redirect_base_url() {
    // Unconfigured returns None
    let config = OAuthConfig { microsoft: None };
    let service = OAuthService::new(config);
    assert_eq!(service.redirect_base_url(), None);

    // Configured returns the base URL
    let config = OAuthConfig {
        microsoft: Some(OAuthProviderConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            redirect_base_url: "http://localhost:9439".to_string(),
        }),
    };
    let service = OAuthService::new(config);
    assert_eq!(service.redirect_base_url(), Some("http://localhost:9439"));
}

#[tokio::test]
async fn test_oauth_service_generates_valid_auth_url() {
    let config = OAuthConfig {
        microsoft: Some(OAuthProviderConfig {
            client_id: "test-id".to_string(),
            client_secret: "test-secret".to_string(),
            redirect_base_url: "http://localhost:9439".to_string(),
        }),
    };
    let service = OAuthService::new(config);

    let (url, state) = service.generate_microsoft_auth_url().await.unwrap();

    // URL must contain required OAuth2 parameters
    assert!(url.contains("client_id=test-id"), "Missing client_id");
    assert!(url.contains("response_type=code"), "Missing response_type");
    assert!(url.contains("code_challenge="), "Missing code_challenge");
    assert!(url.contains("code_challenge_method=S256"), "Missing PKCE method");
    assert!(url.contains("scope="), "Missing scope");

    // State must be non-empty
    assert!(!state.is_empty());
}

#[tokio::test]
async fn test_oauth_service_unique_states() {
    let config = OAuthConfig {
        microsoft: Some(OAuthProviderConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            redirect_base_url: "http://localhost:9439".to_string(),
        }),
    };
    let service = OAuthService::new(config);

    let (_, state1) = service.generate_microsoft_auth_url().await.unwrap();
    let (_, state2) = service.generate_microsoft_auth_url().await.unwrap();

    assert_ne!(state1, state2, "States must be unique per request");
}

#[tokio::test]
async fn test_exchange_code_rejects_unknown_state() {
    let config = OAuthConfig {
        microsoft: Some(OAuthProviderConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            redirect_base_url: "http://localhost:9439".to_string(),
        }),
    };
    let service = OAuthService::new(config);

    let result = service.exchange_code("unknown-state", "some-code").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        OAuthError::NoPendingAuth(s) => assert_eq!(s, "unknown-state"),
        other => panic!("Expected NoPendingAuth, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_refresh_token_rejects_unconfigured() {
    let config = OAuthConfig { microsoft: None };
    let service = OAuthService::new(config);

    let result = service.refresh_token("some-refresh-token").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OAuthError::NotConfigured));
}

// ============================================================================
// XOAUTH2 Token Tests
// ============================================================================

#[test]
fn test_xoauth2_token_format_rfc() {
    // XOAUTH2 format per RFC: user=<email>\x01auth=Bearer <token>\x01\x01
    let token = OAuthService::build_xoauth2_token("user@example.com", "abc123");
    assert_eq!(token, "user=user@example.com\x01auth=Bearer abc123\x01\x01");

    // Must contain exactly two \x01 separators and end with \x01\x01
    let parts: Vec<&str> = token.split('\x01').collect();
    assert_eq!(parts.len(), 4); // user=..., auth=Bearer ..., "", ""
}

#[test]
fn test_xoauth2_authenticator_produces_correct_response() {
    let mut auth = XOAuth2Authenticator::new("test@outlook.com", "my-access-token");
    let response = auth.process(b"");
    assert!(response.starts_with("user=test@outlook.com\x01"));
    assert!(response.contains("auth=Bearer my-access-token"));
}

#[test]
fn test_xoauth2_authenticator_ignores_server_challenge() {
    let mut auth = XOAuth2Authenticator::new("a@b.com", "token");
    let r1 = auth.process(b"server says hello");
    let r2 = auth.process(b"different challenge");
    assert_eq!(r1, r2, "XOAUTH2 must ignore challenge content");
}

// ============================================================================
// OAuth Token Expiry Tests
// ============================================================================

#[test]
fn test_oauth_tokens_not_expired() {
    let now = Utc::now().timestamp();
    let tokens = OAuthTokens {
        access_token: "tok".to_string(),
        refresh_token: "ref".to_string(),
        expires_at: now + 3600, // 1 hour from now
        email: "test@test.com".to_string(),
    };
    assert!(!tokens.is_expired(300)); // 5-min margin
}

#[test]
fn test_oauth_tokens_expired_with_margin() {
    let now = Utc::now().timestamp();
    let tokens = OAuthTokens {
        access_token: "tok".to_string(),
        refresh_token: "ref".to_string(),
        expires_at: now + 200, // Only 3:20 left
        email: "test@test.com".to_string(),
    };
    assert!(tokens.is_expired(300)); // Within 5-min margin
}

#[test]
fn test_oauth_tokens_already_expired() {
    let now = Utc::now().timestamp();
    let tokens = OAuthTokens {
        access_token: "tok".to_string(),
        refresh_token: "ref".to_string(),
        expires_at: now - 60, // Expired 1 minute ago
        email: "test@test.com".to_string(),
    };
    assert!(tokens.is_expired(0));
}

// ============================================================================
// Account Store OAuth Integration Tests
// ============================================================================

#[tokio::test]
async fn test_oauth_account_store_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");
    let store = AccountStore::new(&config_path);
    store.initialize().await.unwrap();

    let account = StoredAccount {
        display_name: "Microsoft Account".to_string(),
        email_address: "user@outlook.com".to_string(),
        provider_type: Some("outlook".to_string()),
        imap: ImapConfig {
            host: "outlook.office365.com".to_string(),
            port: 993,
            username: "user@outlook.com".to_string(),
            password: String::new(),
            use_tls: true,
        },
        smtp: Some(SmtpConfig {
            host: "smtp.office365.com".to_string(),
            port: 587,
            username: "user@outlook.com".to_string(),
            password: String::new(),
            use_tls: true,
            use_starttls: true,
        }),
        oauth_provider: Some("microsoft".to_string()),
        oauth_access_token: Some("access-token-abc".to_string()),
        oauth_refresh_token: Some("refresh-token-xyz".to_string()),
        oauth_token_expiry: Some(1700000000),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    store.add_account(account).await.unwrap();

    let retrieved = store.get_account("user@outlook.com").await.unwrap();
    assert!(retrieved.is_oauth());
    assert_eq!(retrieved.oauth_provider.as_deref(), Some("microsoft"));
    assert_eq!(retrieved.oauth_access_token.as_deref(), Some("access-token-abc"));
    assert_eq!(retrieved.oauth_refresh_token.as_deref(), Some("refresh-token-xyz"));
    assert_eq!(retrieved.oauth_token_expiry, Some(1700000000));
}

#[tokio::test]
async fn test_mixed_accounts_password_and_oauth() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");
    let store = AccountStore::new(&config_path);
    store.initialize().await.unwrap();

    // Add a password-based account
    let password_account = StoredAccount {
        display_name: "Gmail".to_string(),
        email_address: "user@gmail.com".to_string(),
        provider_type: Some("gmail".to_string()),
        imap: ImapConfig {
            host: "imap.gmail.com".to_string(),
            port: 993,
            username: "user@gmail.com".to_string(),
            password: "app-password".to_string(),
            use_tls: true,
        },
        smtp: None,
        oauth_provider: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
        oauth_token_expiry: None,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Add an OAuth account
    let oauth_account = StoredAccount {
        display_name: "Outlook".to_string(),
        email_address: "user@outlook.com".to_string(),
        provider_type: Some("outlook".to_string()),
        imap: ImapConfig {
            host: "outlook.office365.com".to_string(),
            port: 993,
            username: "user@outlook.com".to_string(),
            password: String::new(),
            use_tls: true,
        },
        smtp: None,
        oauth_provider: Some("microsoft".to_string()),
        oauth_access_token: Some("token".to_string()),
        oauth_refresh_token: Some("refresh".to_string()),
        oauth_token_expiry: Some(9999999999),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    store.add_account(password_account).await.unwrap();
    store.add_account(oauth_account).await.unwrap();

    let accounts = store.list_accounts().await.unwrap();
    assert_eq!(accounts.len(), 2);

    let gmail = accounts.iter().find(|a| a.email_address == "user@gmail.com").unwrap();
    assert!(!gmail.is_oauth());
    assert_eq!(gmail.imap.password, "app-password");

    let outlook = accounts.iter().find(|a| a.email_address == "user@outlook.com").unwrap();
    assert!(outlook.is_oauth());
    assert!(outlook.imap.password.is_empty());
}

#[tokio::test]
async fn test_update_preserves_oauth_fields() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");
    let store = AccountStore::new(&config_path);
    store.initialize().await.unwrap();

    let account = StoredAccount {
        display_name: "OAuth Account".to_string(),
        email_address: "user@outlook.com".to_string(),
        provider_type: Some("outlook".to_string()),
        imap: ImapConfig {
            host: "outlook.office365.com".to_string(),
            port: 993,
            username: "user@outlook.com".to_string(),
            password: String::new(),
            use_tls: true,
        },
        smtp: None,
        oauth_provider: Some("microsoft".to_string()),
        oauth_access_token: Some("old-token".to_string()),
        oauth_refresh_token: Some("old-refresh".to_string()),
        oauth_token_expiry: Some(1000),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    store.add_account(account).await.unwrap();

    // Update only the display name and token
    let mut updated = store.get_account("user@outlook.com").await.unwrap();
    updated.display_name = "Updated Name".to_string();
    updated.oauth_access_token = Some("new-token".to_string());
    updated.oauth_token_expiry = Some(2000);
    store.update_account(updated).await.unwrap();

    let retrieved = store.get_account("user@outlook.com").await.unwrap();
    assert_eq!(retrieved.display_name, "Updated Name");
    assert_eq!(retrieved.oauth_access_token.as_deref(), Some("new-token"));
    assert_eq!(retrieved.oauth_refresh_token.as_deref(), Some("old-refresh"));
    assert_eq!(retrieved.oauth_token_expiry, Some(2000));
}
