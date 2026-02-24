# Task ID: 48

**Title:** Update SMTP Client for XOAUTH2 Authentication

**Status:** done

**Dependencies:** 46 ✓, 47 ✓

**Priority:** high

**Description:** Implement R5: Modify lettre SMTP client to use XOAUTH2 for Microsoft accounts

**Details:**

In SMTP logic (`src/dashboard/services/smtp.rs`):

```rust
use lettre::transport::smtp::authentication::Xoauth2;

pub async fn create_smtp_transport(account: &mut Account) -> Result<SmtpTransport> {
    if account.oauth_provider == Some("microsoft".to_string()) {
        refresh_token_if_needed(account, &oauth_service, &encryption).await?;
        let access_token = encryption.decrypt(&account.oauth_access_token)?;
        let xoauth2_token = generate_xoauth2_token(&account.email, &access_token);
        
        let auth = Xoauth2::new(account.email.clone(), access_token);
        let transport = SmtpTransport::starttls_relay(&config.smtp_server)
            ?.credentials(auth)
            .port(587)
            .build();
        Ok(transport)
    } else {
        // Existing password auth
    }
}
```

**Test Strategy:**

Integration test with smtp test server, verify XOAUTH2 SMTP auth succeeds, test with expired tokens triggers refresh
