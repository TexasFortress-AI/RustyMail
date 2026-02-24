# Task ID: 43

**Title:** Add Microsoft OAuth2 Environment Configuration

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Implement R1: Add environment variables for Microsoft OAuth2 app registration and load them at startup

**Details:**

Add to `.env.example`:

```
MICROSOFT_CLIENT_ID=your_azure_app_client_id
MICROSOFT_CLIENT_SECRET=your_azure_app_client_secret
OAUTH_REDIRECT_BASE_URL=http://localhost:9780
```

In `src/dashboard/config.rs`, use `config::Config` or `dotenvy` (v0.15) to load vars. Create `MicrosoftOAuthConfig` struct:

```rust
#[derive(Clone, Debug)]
pub struct MicrosoftOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_base_url: String,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: Vec<String>,
}

impl MicrosoftOAuthConfig {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client_id: env::var("MICROSOFT_CLIENT_ID")?,
            client_secret: env::var("MICROSOFT_CLIENT_SECRET")?,
            redirect_base_url: env::var("OAUTH_REDIRECT_BASE_URL")?,
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            scopes: vec![
                "https://outlook.office365.com/IMAP.AccessAsUser.All".to_string(),
                "https://outlook.office365.com/SMTP.Send".to_string(),
                "offline_access".to_string(),
            ],
        })
    }
}
```

**Test Strategy:**

Unit test config loading with mock env vars, verify scopes contain exact 3 required strings, test missing vars return proper errors
