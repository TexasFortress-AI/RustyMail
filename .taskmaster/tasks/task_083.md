# Task ID: 83

**Title:** Implement OAuth Token Auto-Refresh for Microsoft 365

**Status:** pending

**Dependencies:** None

**Priority:** high

**Description:** Create a background service that automatically refreshes OAuth tokens before they expire to prevent authentication failures

**Details:**

Implement a background task using tokio::time::interval that runs every hour to check oauth_token_expiry in accounts.json. When less than 1 hour remains:

```rust
// In auth_service.rs
pub async fn refresh_oauth_token(account: &Account) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let refresh_request = RefreshTokenRequest {
        grant_type: "refresh_token".to_string(),
        refresh_token: account.refresh_token.clone(),
        client_id: account.client_id.clone(),
        client_secret: account.client_secret.clone(),
        scope: "https://outlook.office365.com/IMAP.AccessAsUser.All offline_access".to_string(),
    };
    
    let response = client.post(&format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", account.tenant_id))
        .form(&refresh_request)
        .send()
        .await?;
        
    if response.status().is_success() {
        let token_response: TokenResponse = response.json().await?;
        AccountService::update_oauth_tokens(
            account.id,
            &token_response.access_token,
            Utc::now() + Duration::seconds(token_response.expires_in)
        ).await?;
        Ok(token_response)
    } else {
        // Log error and notify UI
        error!("Token refresh failed: {}", response.text().await?);
        Err(AuthError::RefreshFailed)
    }
}

// Background task
pub async fn start_token_refresh_service() {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Check hourly
    loop {
        interval.tick().await;
        let accounts = AccountService::get_all_oauth_accounts().await.unwrap();
        for account in accounts {
            if let Some(expiry) = account.oauth_token_expiry {
                if expiry - Utc::now() < Duration::hours(1) {
                    match refresh_oauth_token(&account).await {
                        Ok(_) => info!("Refreshed token for account {}", account.email),
                        Err(e) => error!("Failed to refresh token for {}: {}", account.email, e)
                    }
                }
            }
        }
    }
}
```

Add logging for all refresh attempts and implement UI notifications for refresh failures.

**Test Strategy:**

1. Unit test refresh_oauth_token with mock HTTP client returning success/failure responses
2. Test token expiry calculation logic with various time scenarios
3. Integration test with expired token to verify full refresh cycle
4. Test error handling for network failures and invalid refresh tokens
5. Verify background task runs at correct intervals using tokio test utilities
