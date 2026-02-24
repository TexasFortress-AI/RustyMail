# Task ID: 47

**Title:** Update IMAP Client for XOAUTH2 Authentication

**Status:** done

**Dependencies:** 46 ✓

**Priority:** high

**Description:** Implement R4: Modify async-imap client to use XOAUTH2 for Microsoft accounts

**Details:**

In IMAP connection logic (`src/dashboard/services/imap.rs`):

```rust
pub async fn connect_imap(account: &mut Account, config: &Config) -> Result<ImapStream> {
    let mut imap = async_imap::connect(config.imap_server, Office365ImapStream::new()).await?;
    imap.login("", "").await.map_err(|_| anyhow::anyhow!("Skip login"))?;
    
    if account.oauth_provider == Some("microsoft".to_string()) {
        refresh_token_if_needed(account, &oauth_service, &encryption).await?;
        let access_token = encryption.decrypt(&account.oauth_access_token)?;
        let xoauth2_token = generate_xoauth2_token(&account.email, &access_token);
        imap.authenticate("XOAUTH2", xoauth2_token.as_bytes()).await?;
    } else {
        imap.login(&account.email, &account.imap_password).await?;
    }
    Ok(imap)
}
```

**Test Strategy:**

Integration test with imap test server (greenmail/docker), verify XOAUTH2 auth succeeds for microsoft accounts, test token refresh trigger
