# MCP Account Tools - Quick Start Guide

RustyMail serves **multiple email accounts** through one MCP server. Every email tool names
the account it acts on, so there is no "current account" to switch.

## How accounts are chosen

- `list_accounts` lists the configured accounts.
- Every email tool takes a **required** `account_id`: the account's email address
  (e.g. `you@example.com`). A call without one is refused rather than run against some default.
- Nothing is remembered between calls, so several clients (Claude Desktop, the dashboard, the
  dashboard's assistant) can work on different accounts at the same time without affecting each
  other.

An earlier `set_current_account` tool claimed to switch accounts for later calls but never stored
anything. It was removed in November 2026; pass `account_id` on each call instead.

## `list_accounts`

**Parameters:** None

**Example Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": "you@gmail.com",
      "account_name": "Personal Gmail",
      "email_address": "you@gmail.com",
      "is_default": true
    },
    {
      "id": "you@company.com",
      "account_name": "Work Email",
      "email_address": "you@company.com",
      "is_default": false
    }
  ],
  "count": 2
}
```

## How to Use

### Via Dashboard UI

1. Open the RustyMail Dashboard (`http://127.0.0.1:<REST_PORT>/`; `REST_PORT` is set in `.env`)
2. In the **"MCP Email Tools"** widget, expand `list_accounts` and click **Execute Tool**
3. For any other tool, enter the account's email address in its `account_id` field

### Via Claude Desktop

```
You: "List my email accounts"
Claude: [calls list_accounts]

You: "Show me unread emails in my work account"
Claude: [calls list_emails_by_flag with account_id "you@company.com"]
```

### Via API

```bash
# List accounts
curl -X POST http://127.0.0.1:<REST_PORT>/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"tool": "list_accounts", "parameters": {}}'

# List folders of one account
curl -X POST http://127.0.0.1:<REST_PORT>/api/dashboard/mcp/execute \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"tool": "list_folders", "parameters": {"account_id": "you@company.com"}}'
```

## Troubleshooting

**"account_id parameter is required":**
- Add `account_id` (the account's email address) to the tool call.

**"Account not found" error:**
- Use an email address that `list_accounts` returns.

**No accounts returned:**
- Configure at least one email account: Dashboard → Accounts → Add Account.
