# Task ID: 49

**Title:** Update Account Store for OAuth Fields

**Status:** done

**Dependencies:** 45 ✓

**Priority:** medium

**Description:** Implement R8: Extend StoredAccount struct and account_store.rs for OAuth support

**Details:**

Update `src/dashboard/services/account_store.rs`:

```rust
#[derive(Serialize, Deserialize)]
pub struct StoredAccount {
    // existing fields...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_access_token: Option<String>, // encrypted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_refresh_token: Option<String>, // encrypted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_token_expiry: Option<DateTime<Utc>>,
}

impl AccountStore {
    pub async fn sync_oauth_account(&self, account: &Account) -> Result<()> {
        let stored = self.load().await?;
        if let Some(mut stored_acc) = stored.accounts.iter_mut()
            .find(|a| a.email == account.email) {
            stored_acc.oauth_provider = account.oauth_provider.clone();
            // sync other oauth fields
            self.save(&stored).await?;
        }
    }
}
```

**Test Strategy:**

Unit tests for JSON serialization of OAuth fields, test file sync preserves encrypted tokens, test missing fields handled gracefully
