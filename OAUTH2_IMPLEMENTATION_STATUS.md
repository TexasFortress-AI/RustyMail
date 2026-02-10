# Microsoft 365 OAuth2 / XOAUTH2 Implementation Status

## Current State (2026-02-10)

### What Has Been Implemented

1. **XOAUTH2 Authenticator** (`src/imap/xoauth2.rs`)
   - Implements the `Authenticator` trait for async-imap
   - Encodes credentials in XOAUTH2 format: `base64(user={email}\x01auth=Bearer {token}\x01\x01)`
   - Ready for use with Microsoft 365 and Gmail OAuth2

2. **OAuth2 Client Module** (`src/imap/oauth2.rs`)
   - `MicrosoftOAuth2Config` - Configuration for Microsoft 365 OAuth2
   - `MicrosoftOAuth2Client` - Client for token operations
   - `StoredToken` - Token with expiry tracking and refresh logic
   - Token refresh logic for expired access tokens

3. **Session-Level XOAUTH2 Support** (`src/imap/session.rs`)
   - Added `connect_with_xoauth2()` method to `AsyncImapSessionWrapper`
   - Uses `client.authenticate("XOAUTH2", authenticator)` for OAuth2 authentication

4. **Client-Level XOAUTH2 Support** (`src/imap/client.rs`)
   - Added `connect_with_xoauth2()` method to `ImapClient`
   - Added `connect_with_xoauth2_and_timeout()` for custom timeouts

5. **accounts.json as Single Source of Truth** (`src/main.rs`)
   - Modified main.rs to load credentials from `accounts.json` instead of environment variables
   - Created `AccountStore` to read from `config/accounts.json`

### Dependencies Added

- `oauth2 = { version = "4.4", features = ["reqwest", "rustls-tls"] }` (in Cargo.toml)
- `url = "2.5"` (for URL encoding)

## Current Blocker

Microsoft 365 has **Basic Authentication disabled** at the tenant level. This blocks:
- Regular username/password LOGIN command
- App Passwords (which still use Basic Auth under the hood)

Error message:
```
AuthFailed:LogonDenied-BasicAuthBlocked
```

## Solution Paths

### Path 1: Enable Basic Auth at Tenant Level (Quick Fix)

**Prerequisites:** Admin access to Exchange Online

**Steps:**

1. Install Exchange Online PowerShell module:
   ```powershell
   Install-Module -Name ExchangeOnlineManagement -Force
   ```

2. Connect to Exchange Online:
   ```powershell
   Connect-ExchangeOnline -UserPrincipalName admin@mleehealthcare.ai
   ```

3. Create authentication policy allowing Basic Auth for IMAP:
   ```powershell
   New-AuthenticationPolicy -Name "AllowBasicAuthIMAP"
   Set-AuthenticationPolicy -Identity "AllowBasicAuthIMAP" -AllowBasicAuthImap $true
   ```

4. Apply to the specific user:
   ```powershell
   Set-User -Identity "jobs@mleehealthcare.ai" -AuthenticationPolicy "AllowBasicAuthIMAP"
   ```

5. Or apply to entire organization (NOT recommended for security):
   ```powershell
   Set-OrganizationConfig -AuthenticationPolicies "AllowBasicAuthIMAP"
   ```

**Note:** Microsoft is deprecating Basic Auth. This is a temporary fix.

### Path 2: Complete OAuth2/XOAUTH2 Implementation (Recommended)

This requires setting up an Azure AD app registration and implementing the OAuth2 flow.

#### Step 2.1: Azure AD App Registration

1. Go to https://portal.azure.com → Azure Active Directory → App registrations
2. Click "New registration"
3. Configure:
   - **Name:** RustyMail
   - **Supported account types:** Accounts in this organizational directory only
   - **Redirect URI:** None (we'll use Device Code Flow)
4. Click "Register"
5. Note the **Application (client) ID** and **Directory (tenant) ID**

#### Step 2.2: Add API Permissions

1. In the app registration, go to "API permissions"
2. Click "Add a permission"
3. Select "Microsoft Graph" → "Delegated permissions"
4. Add these permissions:
   - `IMAP.AccessAsUser.All` (or the Exchange-specific scope)
   - `offline_access` (for refresh tokens)
5. Click "Grant admin consent"

#### Step 2.3: Configure .env

Add to `.env`:
```bash
MICROSOFT_CLIENT_ID=your-client-id-here
MICROSOFT_TENANT_ID=your-tenant-id-here
```

#### Step 2.4: Implement Device Code Flow (TODO)

Current OAuth2 implementation has token refresh but NOT initial token acquisition.

Need to implement:

1. **Device Code Flow** for initial token acquisition:
   ```rust
   // Pseudocode - needs to be implemented
   pub async fn start_device_code_flow(&self) -> Result<DeviceCodeResponse, OAuth2Error> {
       // POST to https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode
       // with client_id and scope
       // Returns device_code, user_code, verification_uri, expires_in, interval
   }

   pub async fn poll_for_token(&self, device_code: &str) -> Result<TokenResponse, OAuth2Error> {
       // Poll https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token
       // with grant_type=urn:ietf:params:oauth:grant-type:device_code
       // Returns access_token, refresh_token, expires_in
   }
   ```

2. **Token Storage in accounts.json**:
   - Add `oauth_access_token`, `oauth_refresh_token`, `oauth_token_expiry` to `ImapConfig`
   - Implement encryption for tokens (similar to password encryption)

3. **Startup Token Check**:
   - On startup, check if token is expired
   - If expired, use refresh_token to get new access_token
   - If refresh fails, prompt user to re-authenticate via device code

4. **IMAP Connection Selection**:
   - Add `auth_method` field to account config
   - If `auth_method == "oauth2"`, use `ImapClient::connect_with_xoauth2()`
   - If `auth_method == "password"`, use `ImapClient::connect()`

#### Step 2.5: Web UI for OAuth2 Setup

Add a dashboard page:
1. "Connect Microsoft 365 Account" button
2. Show device code and verification URL
3. Poll for completion
4. Store tokens on success

## Code Changes Needed

### Files Modified/Created:

1. `Cargo.toml` - Added oauth2 and url dependencies
2. `src/imap/xoauth2.rs` - XOAUTH2 authenticator (DONE)
3. `src/imap/oauth2.rs` - OAuth2 client with refresh (PARTIAL - needs device code flow)
4. `src/imap/session.rs` - Added `connect_with_xoauth2()` (DONE)
5. `src/imap/client.rs` - Added `connect_with_xoauth2()` (DONE)
6. `src/imap/mod.rs` - Added module exports (DONE)
7. `src/main.rs` - Changed to use accounts.json (DONE)

### Files Still Need Changes:

1. `src/imap/oauth2.rs` - Add device code flow
2. `src/dashboard/services/account_store.rs` - Add OAuth token fields to ImapConfig
3. `src/dashboard/api/accounts.rs` - Add OAuth2 setup endpoints
4. `frontend/rustymail-app-main/` - Add OAuth2 UI flow

## Testing Plan

1. **Basic Auth Test** (if enabled):
   ```bash
   # Update .env with App Password
   IMAP_PASS=your-app-password
   ```

2. **OAuth2 Test** (when implemented):
   ```bash
   # Update .env with OAuth credentials
   MICROSOFT_CLIENT_ID=xxx
   MICROSOFT_TENANT_ID=xxx

   # Add to accounts.json
   "imap": {
     "auth_method": "oauth2",
     "oauth_provider": "microsoft365",
     "oauth_access_token": "...",
     "oauth_refresh_token": "..."
   }
   ```

## Next Session Instructions

To continue this work:

1. **Verify Basic Auth is enabled** in Exchange Online (ask admin to run PowerShell commands above)

2. **Test with App Password**:
   - Verify `.env` has `config/accounts.json` as the source of truth
   - Configure `accounts.json` with email, app password, `auth_method: "app_password"`
   - Restart backend and test IMAP connection

3. **If Basic Auth doesn't work**, implement OAuth2 device code flow:
   - Complete `src/imap/oauth2.rs` with device code flow
   - Add web UI for OAuth2 authentication
   - Test with Microsoft 365

4. **Verify Frontend Integration**:
   - Ensure frontend can display emails
   - Add OAuth2 setup UI

## Relevant Documentation

- [Microsoft OAuth2 for IMAP](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth)
- [async-imap Authenticator trait](https://docs.rs/async-imap/0.8.0/async_imap/trait.Authenticator.html)
- [OAuth2 Device Code Flow](https://learn.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-device-code)

## Current Git Status

Branch: main
Uncommitted changes:
- OAuth2 implementation files
- Cargo.toml updates
- main.rs changes for accounts.json

**ACTION REQUIRED:** Commit these changes before next session.
