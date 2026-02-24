# Task ID: 45

**Title:** Implement OAuth2 API Endpoints

**Status:** done

**Dependencies:** 44 ✓

**Priority:** high

**Description:** Implement R3: Add REST API endpoints for Microsoft OAuth authorize and callback

**Details:**

In `src/dashboard/api/oauth.rs`:

```rust
#[get("/api/oauth/microsoft/authorize?state=<state>")]
pub async fn microsoft_authorize(
    state: String,
    oauth_service: web::Data<MicrosoftOAuthService>,
) -> impl Responder {
    let auth_url = oauth_service.authorize_url(&state);
    HttpResponse::Ok().json(AuthUrlResponse { url: auth_url })
}

#[get("/api/oauth/callback/microsoft?code={code}&state={state}")]
pub async fn microsoft_callback(
    code: String,
    state: String,
    oauth_service: web::Data<MicrosoftOAuthService>,
    account_store: web::Data<AccountStore>,
) -> impl Responder {
    let token_result = oauth_service
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(get_pkce_verifier(state)) // from session
        .request_async(async_http_client).await;
    // Encrypt and store tokens using existing encryption service
    // Create/update account with oauth_provider = "microsoft"
    HttpResponse::Ok().json("Account linked successfully")
}
```

**Test Strategy:**

Integration tests with wiremock for OAuth endpoints, test auth URL generation, test token exchange with mock token response, verify account creation
