# Task ID: 44

**Title:** Create OAuth2 Service with PKCE Support

**Status:** done

**Dependencies:** 43 ✓

**Priority:** high

**Description:** Implement R2: Create Rust service for OAuth2 authorization code flow with PKCE using oauth2 crate v4.4

**Details:**

Add to Cargo.toml: `oauth2 = "4.4"`, `rand = "0.8"`, `sha2 = "0.10"`, `base64 = "0.21"`

Create `src/dashboard/services/oauth_microsoft.rs`:

```rust
use oauth2::{
    AuthorizationCode, AuthUrl, ClientId, ClientSecret, CodeChallenge, CodeVerifier,
    PkceCodeChallenge, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::http::Request;

pub struct MicrosoftOAuthService {
    client: BasicClient,
}

impl MicrosoftOAuthService {
    pub fn new(config: MicrosoftOAuthConfig) -> Self {
        let client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
        )
        .set_auth_uri(AuthUrl::new(config.auth_url.to_string()).unwrap())
        .set_token_uri(TokenUrl::new(config.token_url.to_string()).unwrap())
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/oauth/callback/microsoft", config.redirect_base_url)).unwrap(),
        );
        Self { client }
    }

    pub fn authorize_url(&self, state: impl AsRef<str>) -> String {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        // Store pkce_verifier in session/redis with state
        self.client
            .authorize_url(AuthorizationCode::new("".to_string()))
            .add_extra_params(&[("state", state.as_ref())])
            .set_pkce_challenge(pkce_challenge)
            .url().to_string()
    }
}
```

**Test Strategy:**

Mock HTTP client, test authorize_url generates correct URL with PKCE challenge and required scopes, verify state parameter inclusion
