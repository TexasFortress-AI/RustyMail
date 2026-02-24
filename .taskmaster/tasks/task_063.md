# Task ID: 63

**Title:** Upgrade async-imap from 0.8 to 0.11+

**Status:** deferred

**Dependencies:** 47 ✓, 46 ✓

**Priority:** medium

**Description:** Upgrade async-imap dependency from version 0.8 to 0.11+ to address unmaintained async-std and unsound ouroboros dependencies, while ensuring critical XOAUTH2 authentication behavior is preserved including the greeting consumption workaround.

**Details:**

1. **Update Cargo.toml dependencies**:
```toml
[dependencies]
async-imap = "0.11.1"
# Remove async-std if it was a direct dependency
# Verify tokio is used as the async runtime
```

2. **Critical XOAUTH2 authenticate() verification**:
   - The current implementation has a workaround for greeting consumption that MUST be preserved
   - Review the authenticate() method implementation in 0.11.1 to ensure it still:
     a) Properly consumes server greetings before authentication
     b) Handles the XOAUTH2 SASL mechanism correctly
     c) Maintains compatibility with our token format from `generate_xoauth2_token()`

3. **Update IMAP connection logic** in `src/dashboard/services/imap.rs`:
```rust
// Verify the connection pattern still works
pub async fn connect_imap(account: &mut Account, config: &Config) -> Result<ImapStream> {
    // Check if async-imap 0.11+ still supports our connection approach
    let mut imap = async_imap::connect(config.imap_server, tls_stream).await?;
    
    // CRITICAL: Verify greeting consumption workaround still functions
    // The empty login trick may need adjustment
    imap.login("", "").await.map_err(|_| anyhow::anyhow!("Skip login"))?;
    
    if account.oauth_provider == Some("microsoft".to_string()) {
        // Ensure XOAUTH2 authentication still works
        let xoauth2_token = generate_xoauth2_token(&account.email, &access_token);
        imap.authenticate("XOAUTH2", xoauth2_token).await?;
    }
}
```

4. **API changes to address**:
   - Check for any changes in the `Session` type or its methods
   - Verify `select()`, `fetch()`, `search()`, and other IMAP commands still have the same signatures
   - Update any error handling if error types have changed
   - Check if the TLS stream setup needs modifications

5. **Remove ouroboros workarounds** if any exist in the codebase:
   - Search for any self-referential struct patterns that were using ouroboros
   - Replace with safe alternatives provided by async-imap 0.11+

6. **Update all MCP tools using IMAP**:
   - Review and update any IMAP usage in MCP tool implementations
   - Ensure all tools maintain their current functionality

**Test Strategy:**

1. **Verify XOAUTH2 authentication behavior**:
   - Create integration test specifically for XOAUTH2 auth flow
   - Test with real Microsoft OAuth tokens to ensure greeting consumption works
   - Verify the authenticate() method properly handles the SASL XOAUTH2 mechanism
   - Test error cases (invalid tokens, expired tokens)

2. **Test all IMAP operations**:
   - Login with username/password (non-OAuth accounts)
   - OAuth XOAUTH2 authentication for Microsoft accounts
   - Folder selection and listing
   - Email fetching (headers and full messages)
   - Search functionality
   - Flag operations (mark read/unread)

3. **Test all MCP tools that use IMAP**:
   - Run each MCP tool that interacts with IMAP
   - Verify email listing, reading, searching still work
   - Test with both OAuth and non-OAuth accounts

4. **Performance and stability testing**:
   - Test connection pooling behavior
   - Verify no memory leaks with long-running connections
   - Test reconnection after network interruptions

5. **Regression testing**:
   ```bash
   cargo test --all-features
   cargo test --test imap_integration_tests
   ```

6. **Manual testing checklist**:
   - Connect to Gmail with OAuth
   - Connect to Outlook/Office365 with OAuth
   - Connect to standard IMAP server with password
   - Verify the greeting consumption workaround still prevents authentication failures
