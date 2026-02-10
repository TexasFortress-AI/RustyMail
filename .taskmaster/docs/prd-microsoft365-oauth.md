# PRD: Microsoft 365 OAuth2 Email Account Authorization

## Overview

RustyMail needs to support Microsoft 365 (M365) email account authorization via OAuth2. Microsoft has deprecated basic authentication (username/password) for Exchange Online, making OAuth2 the only viable authentication method for M365 accounts. The database schema already has OAuth columns (`oauth_provider`, `oauth_access_token`, `oauth_refresh_token`, `oauth_token_expiry`) and provider templates for Outlook domains with `supports_oauth: true`, but no OAuth2 flow is implemented.

## Goals

1. Enable users to link Microsoft 365 email accounts via OAuth2 authorization code flow with PKCE
2. Use XOAUTH2 for IMAP and SMTP authentication with M365 accounts
3. Automatically refresh expired access tokens using stored refresh tokens
4. Integrate OAuth-linked accounts into the existing multi-account system seamlessly

## Non-Goals

- Google OAuth2 (future work, separate PRD)
- Admin consent / multi-tenant enterprise deployment
- Microsoft Graph API for email (we use IMAP/SMTP only)

## Technical Context

### Existing Infrastructure
- **Provider templates**: Outlook/Hotmail/Live already configured in `migrations/003_create_provider_templates.sql` with `outlook.office365.com` IMAP and `smtp.office365.com` SMTP
- **Database schema**: `accounts` table in `migrations/001_create_schema.sql` has `oauth_provider`, `oauth_access_token`, `oauth_refresh_token`, `oauth_token_expiry` columns
- **Encryption**: AES-256-GCM encryption in `src/dashboard/services/encryption.rs` can encrypt OAuth tokens at rest
- **Account service**: `src/dashboard/services/account.rs` and `src/dashboard/services/account_store.rs` handle account CRUD
- **IMAP client**: `async-imap` crate (v0.8.0) supports XOAUTH2 authentication
- **SMTP client**: `lettre` crate (v0.11) supports XOAUTH2 authentication
- **API endpoints**: Account management REST API in `src/dashboard/api/accounts.rs`
- **Frontend**: Svelte-based WebUI in `webui/` directory

### Microsoft OAuth2 Endpoints
- Authorization: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`
- Token: `https://login.microsoftonline.com/common/oauth2/v2.0/token`
- Required scopes: `https://outlook.office365.com/IMAP.AccessAsUser.All`, `https://outlook.office365.com/SMTP.Send`, `offline_access`

## Requirements

### R1: Environment Configuration
Add environment variables for Microsoft OAuth2 app registration:
- `MICROSOFT_CLIENT_ID` - Azure AD app client ID
- `MICROSOFT_CLIENT_SECRET` - Azure AD app client secret
- `OAUTH_REDIRECT_BASE_URL` - Base URL for OAuth callbacks (e.g., `http://localhost:9780`)

These must be added to `.env.example` with documentation. The OAuth configuration should be loaded alongside other configuration at startup.

### R2: OAuth2 Backend Service
Create a Rust service module for handling the OAuth2 authorization code flow with PKCE:
- Generate authorization URL with state parameter, PKCE code verifier/challenge, and required scopes
- Exchange authorization code for access token + refresh token
- Store tokens encrypted in the database using the existing encryption service
- Handle token refresh when access tokens expire
- The `oauth2` crate should be used for the OAuth2 protocol implementation

### R3: OAuth2 API Endpoints
Add REST API endpoints:
- `GET /api/oauth/microsoft/authorize` - Returns the authorization URL for the frontend to redirect to
- `GET /api/oauth/callback/microsoft` - Handles the OAuth callback, exchanges code for tokens, creates/updates the account
- Both endpoints must be protected by the existing API key authentication

### R4: XOAUTH2 IMAP Authentication
Modify the IMAP connection logic to support XOAUTH2:
- When an account has `oauth_provider = "microsoft"`, use XOAUTH2 instead of LOGIN
- The XOAUTH2 token format is: `user=<email>\x01auth=Bearer <access_token>\x01\x01`
- Before connecting, check if the access token is expired and refresh if needed
- If refresh fails, mark the account as needing re-authorization

### R5: XOAUTH2 SMTP Authentication
Modify the SMTP connection logic to support XOAUTH2:
- When an account has `oauth_provider = "microsoft"`, use XOAUTH2 for SMTP authentication via lettre
- Same token format and refresh logic as IMAP

### R6: Token Refresh Logic
Implement automatic token refresh:
- Check token expiry before each IMAP/SMTP connection
- If token expires within 5 minutes, proactively refresh
- On refresh, update the encrypted tokens and expiry in the database
- If refresh token is revoked/expired, set `last_error` on the account indicating re-authorization is needed

### R7: Frontend OAuth Flow
Add Microsoft OAuth account linking to the WebUI:
- Add a "Sign in with Microsoft" button on the account creation/linking page
- Button triggers a redirect to the authorization URL from R3
- After callback completes, the frontend shows the newly linked account
- Display OAuth-linked accounts differently (show provider badge, no password fields)
- Show re-authorization prompt when an OAuth account's tokens are revoked

### R8: Account Store OAuth Integration
Update the file-based account store (`account_store.rs`) to handle OAuth accounts:
- OAuth accounts should store `oauth_provider` field instead of IMAP/SMTP passwords
- The `StoredAccount` struct needs OAuth-related fields
- Sync OAuth fields between file storage and database

### R9: Unit Tests
Add tests for:
- OAuth2 URL generation with correct scopes and PKCE
- Token exchange mock (mock HTTP responses)
- Token refresh logic
- XOAUTH2 token format generation
- Account store with OAuth fields
- Expired token detection and refresh triggering
