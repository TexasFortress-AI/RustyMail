# MCP Account Tools - Quick Start Guide

## What's New?

RustyMail MCP server now supports **multiple email accounts**! You can list accounts and switch between them using two new MCP tools.

## New MCP Tools

### 1. `list_accounts`
Lists all configured email accounts.

**Parameters:** None

**Example Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": "abc-123",
      "account_name": "Personal Gmail",
      "email_address": "you@gmail.com",
      "is_default": true
    },
    {
      "id": "def-456",
      "account_name": "Work Email",
      "email_address": "you@company.com",
      "is_default": false
    }
  ],
  "count": 2
}
```

### 2. `set_current_account`
Sets the current account for all subsequent email operations.

**Parameters:**
- `account_id` (required): The ID of the account to switch to

**Example Request:**
```json
{
  "tool": "set_current_account",
  "parameters": {
    "account_id": "abc-123"
  }
}
```

**Example Response:**
```json
{
  "success": true,
  "message": "Current account set to: abc-123",
  "data": {
    "account_id": "abc-123",
    "account_name": "Personal Gmail",
    "email_address": "you@gmail.com"
  }
}
```

## How to Use

### Via Dashboard UI

1. Open the RustyMail Dashboard (http://localhost:9438)
2. Find the **"MCP Email Tools"** widget
3. Expand the `list_accounts` tool and click **Execute Tool**
4. Note the `id` of the account you want to use
5. Expand the `set_current_account` tool
6. Enter the account ID in the `account_id` field
7. Click **Execute Tool**

### Via Claude Desktop

If you've configured RustyMail as an MCP server in Claude Desktop:

```
You: "List my email accounts"
Claude: [Shows your accounts]

You: "Switch to my work email"
Claude: [Switches to work account]

You: "Show me unread emails"
Claude: [Shows unread emails from work account]
```

### Via API

```bash
# List accounts
curl -X POST http://localhost:9437/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"tool": "list_accounts", "parameters": {}}'

# Set current account
curl -X POST http://localhost:9437/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"tool": "set_current_account", "parameters": {"account_id": "abc-123"}}'
```

## Workflow Example

```
1. list_accounts          → See all your email accounts
2. set_current_account    → Switch to "Work Email"
3. list_folders           → See folders in work account
4. search_emails          → Search work emails
5. set_current_account    → Switch to "Personal Gmail"
6. list_folders           → See folders in personal account
```

## Important Notes

- Each MCP session maintains its own "current account" context
- If you don't set a current account, the default account is used
- Account switching is instant and doesn't require reconnection
- All existing MCP tools (list_folders, search_emails, etc.) will use the current account

## Troubleshooting

**"Account not found" error:**
- Make sure you're using a valid account ID from `list_accounts`
- Account IDs are UUIDs (e.g., "abc-123-def-456"), not account names

**"Account service not available" error:**
- The server may not be fully initialized
- Wait a few seconds and try again

**No accounts returned:**
- You need to configure at least one email account via the Dashboard UI
- Go to Dashboard → Accounts → Add Account

## Next Steps

- Configure multiple email accounts in the Dashboard
- Try switching between accounts using the MCP tools
- Use the account context with other email tools (list_folders, search_emails, etc.)

## Questions?

See the full documentation: [MCP Account Management](./mcp-account-management.md)
