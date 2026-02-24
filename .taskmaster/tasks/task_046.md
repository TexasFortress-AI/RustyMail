# Task ID: 46

**Title:** Implement XOAUTH2 Token Formatter and Refresh Logic

**Status:** done

**Dependencies:** 44 ✓, 45 ✓

**Priority:** high

**Description:** Implement R4,R5,R6: Create XOAUTH2 token formatter and automatic token refresh service

**Details:**

Create `src/dashboard/services/xoauth2.rs`:

```rust
pub fn generate_xoauth2_token(email: &str, access_token: &str) -> String {
    let token = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
    base64::encode(token.as_bytes())
}

pub async fn refresh_token_if_needed(
    account: &mut Account,
    oauth_service: &MicrosoftOAuthService,
    encryption: &EncryptionService,
) -> Result<bool> {
    let expiry = account.oauth_token_expiry;
    if expiry > Utc::now() + Duration::minutes(5) {
        return Ok(false); // No refresh needed
    }
    let refresh_token = encryption.decrypt(&account.oauth_refresh_token)?;
    let new_token = oauth_service
        .refresh_token(&RefreshToken::new(refresh_token))
        .request_async(async_http_client)
        .await?;
    account.oauth_access_token = encryption.encrypt(&new_token.access_token().secret())?;
    account.oauth_refresh_token = encryption.encrypt(&new_token.refresh_token().unwrap().secret())?;
    account.oauth_token_expiry = Utc::now() + Duration::seconds(new_token.expires_in().unwrap_or(3600) as i64);
    account_store.update(account).await?;
    Ok(true)
}
```

**Test Strategy:**

Unit tests for XOAUTH2 format (verify exact byte sequence), test refresh logic with mock token responses, test expiry threshold (5min buffer)
