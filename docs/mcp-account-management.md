# MCP Account Management Implementation

## Overview

This document describes the implementation of account management tools for the RustyMail MCP (Model Context Protocol) server. These tools enable multi-account support through the MCP interface, mirroring the account context pattern used in the web UI.

## Problem Statement

The RustyMail backend supports multiple email accounts, but the MCP interface did not expose account selection functionality. All MCP tools operated on a single "default" or "first" account, making it impossible for:
- Claude Desktop users to work with multiple accounts
- Dashboard Email Assistant to switch between accounts
- MCP clients to specify which account to operate on

## Solution Design

Instead of adding an `account_id` parameter to every MCP tool (which would break existing tools), we implemented a **session-based account context** pattern:

1. **`list_accounts`** - Lists all configured email accounts
2. **`set_current_account`** - Sets the "current account" in the MCP session state
3. All other tools use the current account from session state (future enhancement)

This mirrors the React Context pattern used in the web UI, where users select an account via dropdown and all operations use that account.

## Implementation Details

### 1. MCP State Extension

**File:** `src/mcp/types.rs`

Added `current_account_id` field to `McpPortState`:

```rust
pub struct McpPortState {
    pub selected_folder: Option<String>,
    pub current_account_id: Option<String>,  // NEW
    session_id: Option<String>,
    session_manager: Arc<SessionManager>,
    pub cache_service: Option<Arc<CacheService>>,
}
```

This field stores the currently selected account ID for the MCP session.

### 2. MCP Tools

**File:** `src/mcp_port.rs`

#### `list_accounts_tool`
- Returns current account context information
- Delegates full account listing to dashboard API (which has access to AccountService)
- Response includes `current_account_id` from session state

```rust
pub async fn list_accounts_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    _params: Option<Value>,
) -> Result<Value, JsonRpcError>
```

#### `set_current_account_tool`
- Accepts `account_id` parameter
- Sets `current_account_id` in MCP session state
- Logs the account switch for debugging

```rust
pub async fn set_current_account_tool(
    _session: Arc<dyn AsyncImapOps>,
    state: Arc<TokioMutex<McpPortState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError>
```

Both tools are registered in `create_mcp_tool_registry()`.

### 3. Dashboard API Integration

**File:** `src/dashboard/api/handlers.rs`

#### `list_mcp_tools` Handler
Added tool definitions:

```json
{
  "name": "list_accounts",
  "description": "List all configured email accounts",
  "parameters": {}
}
```

```json
{
  "name": "set_current_account",
  "description": "Set the current account for email operations",
  "parameters": {
    "account_id": "Account ID to set as current"
  }
}
```

#### `execute_mcp_tool` Handler

**list_accounts implementation:**
- Locks `account_service` from `DashboardState`
- Calls `account_service.list_accounts()`
- Returns array of `Account` objects with count

**set_current_account implementation:**
- Validates `account_id` parameter exists
- Locks `account_service` from `DashboardState`
- Calls `account_service.get_account(account_id)` to verify account exists
- Returns success with account details (name, email address)

### 4. Frontend Integration

**File:** `frontend/rustymail-app-main/src/dashboard/components/McpTools.tsx`

No changes required! The component already:
- Dynamically fetches tools from `/api/dashboard/mcp/tools`
- Renders parameters based on tool definition
- Executes tools via `/api/dashboard/mcp/execute`

The new account management tools will automatically appear in the UI.

## Usage Examples

### Via Dashboard UI

1. Open the Dashboard
2. Navigate to the "MCP Email Tools" widget
3. Find "list_accounts" tool and click to expand
4. Click "Execute Tool" to see all accounts
5. Find "set_current_account" tool
6. Enter an account ID in the `account_id` parameter field
7. Click "Execute Tool" to set the current account

### Via MCP Protocol (HTTP)

**List Accounts:**
```bash
curl -X POST http://localhost:9437/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "list_accounts",
    "parameters": {}
  }'
```

**Set Current Account:**
```bash
curl -X POST http://localhost:9437/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "set_current_account",
    "parameters": {
      "account_id": "account-uuid-here"
    }
  }'
```

### Via Claude Desktop (MCP Stdio)

Once configured in Claude Desktop's MCP settings:

```
User: "List my email accounts"
Claude: [calls list_accounts tool]

User: "Switch to my work email account"
Claude: [calls set_current_account with the work account ID]
```

## Response Formats

### list_accounts Response

```json
{
  "success": true,
  "data": [
    {
      "id": "account-uuid-1",
      "account_name": "Personal Gmail",
      "email_address": "user@gmail.com",
      "provider_type": "gmail",
      "imap_host": "imap.gmail.com",
      "imap_port": 993,
      "imap_use_tls": true,
      "is_active": true,
      "is_default": true
    },
    {
      "id": "account-uuid-2",
      "account_name": "Work Email",
      "email_address": "user@company.com",
      "provider_type": "custom",
      "imap_host": "mail.company.com",
      "imap_port": 993,
      "imap_use_tls": true,
      "is_active": true,
      "is_default": false
    }
  ],
  "count": 2,
  "tool": "list_accounts"
}
```

### set_current_account Response (Success)

```json
{
  "success": true,
  "message": "Current account set to: account-uuid-1",
  "data": {
    "account_id": "account-uuid-1",
    "account_name": "Personal Gmail",
    "email_address": "user@gmail.com"
  },
  "tool": "set_current_account"
}
```

### set_current_account Response (Error)

```json
{
  "success": false,
  "error": "Account not found: No account with ID 'invalid-id'",
  "tool": "set_current_account"
}
```

## Future Enhancements

### Phase 1: Current Implementation ✅
- [x] Add `current_account_id` to MCP state
- [x] Implement `list_accounts` tool
- [x] Implement `set_current_account` tool
- [x] Dashboard API integration
- [x] Frontend widget support

### Phase 2: Account-Aware Operations (TODO)
- [ ] Modify existing MCP tools to use `current_account_id` from state
- [ ] Update IMAP session factory to accept account ID
- [ ] Add account validation to email operations
- [ ] Handle account switching mid-session

### Phase 3: Advanced Features (TODO)
- [ ] Add `get_current_account` tool to query current account
- [ ] Support account-specific folder lists
- [ ] Implement account-specific search scopes
- [ ] Add account switching notifications via SSE

## Testing

### Manual Testing

1. **Start the server:**
   ```bash
   ./target/release/rustymail-server
   ```

2. **Configure accounts via Dashboard UI:**
   - Navigate to http://localhost:9438
   - Add at least 2 email accounts

3. **Test via script:**
   ```bash
   ./test-account-tools.sh
   ```

4. **Test via Dashboard UI:**
   - Open MCP Email Tools widget
   - Execute `list_accounts`
   - Execute `set_current_account` with a valid account ID

### Automated Testing (TODO)

```rust
#[tokio::test]
async fn test_list_accounts_tool() {
    // Setup test state with mock accounts
    // Call list_accounts_tool
    // Assert response contains accounts
}

#[tokio::test]
async fn test_set_current_account_tool() {
    // Setup test state
    // Call set_current_account_tool with valid ID
    // Assert current_account_id is set in state
}
```

## Architecture Benefits

1. **No Breaking Changes**: Existing MCP tools continue to work without modification
2. **Mirrors Web UI**: Uses the same account context pattern as the frontend
3. **Clean Separation**: Account management is separate from email operations
4. **Extensible**: Easy to add account-aware behavior to existing tools
5. **Session-Based**: Each MCP session maintains its own account context
6. **Type-Safe**: Leverages Rust's type system for safety

## Security Considerations

- Account IDs are UUIDs, not sequential integers (prevents enumeration)
- Account passwords are never returned in API responses (`#[serde(skip_serializing)]`)
- Account validation happens before setting current account
- MCP tools require authentication (API key or session)

## Related Files

- `src/mcp/types.rs` - MCP state definition
- `src/mcp_port.rs` - MCP tool implementations
- `src/dashboard/api/handlers.rs` - Dashboard API handlers
- `src/dashboard/services/account.rs` - Account service
- `frontend/rustymail-app-main/src/dashboard/components/McpTools.tsx` - UI widget
- `test-account-tools.sh` - Test script

## References

- [MCP Specification](https://modelcontextprotocol.io/)
- [RustyMail Architecture Documentation](../README.md)
- [Account Service Implementation](../src/dashboard/services/account.rs)
