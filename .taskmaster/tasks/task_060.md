# Task ID: 60

**Title:** Fix OAuth Redirect UX Issue

**Status:** done

**Dependencies:** None

**Priority:** low

**Description:** Resolve the UI bug where successful OAuth authentication redirects to a raw JSON response instead of returning to the RustyMail interface.

**Details:**

1. Modify OAuth callback handler to redirect to UI:
```rust
async fn oauth_callback(query: OAuthResponse) -> impl Responder {
    // Process OAuth response
    let result = process_oauth_token(query).await;
    
    // Redirect to UI with status
    if result.success {
        HttpResponse::Found()
            .header("Location", "/accounts?oauth=success&email={}", result.email)
            .finish()
    } else {
        HttpResponse::Found()
            .header("Location", "/accounts?oauth=failed&error={}", result.error)
            .finish()
    }
}
```
2. Update frontend to handle OAuth status parameters
3. Add success/error toast notifications
4. Implement proper error handling for OAuth failures

**Test Strategy:**

1. Test successful OAuth flow returns to UI
2. Test failed OAuth shows appropriate error
3. Verify no sensitive data in URL parameters
4. Test with multiple OAuth providers
5. Verify browser back button behavior
