#!/bin/bash
# Test script for new MCP account management tools

set -e

API_BASE="http://localhost:9437/api/dashboard"
API_KEY="${DASHBOARD_API_KEY:-test-api-key-12345}"

echo "Testing MCP Account Management Tools"
echo "====================================="
echo ""

# Test 1: List MCP Tools (should include new account tools)
echo "1. Listing MCP tools..."
curl -s -X GET "${API_BASE}/mcp/tools" \
  -H "X-API-Key: ${API_KEY}" \
  -H "Content-Type: application/json" | jq '.tools[] | select(.name | contains("account"))'
echo ""

# Test 2: List Accounts
echo "2. Testing list_accounts tool..."
curl -s -X POST "${API_BASE}/mcp/execute" \
  -H "X-API-Key: ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "list_accounts",
    "parameters": {}
  }' | jq '.'
echo ""

# Test 3: Set Current Account (will fail if no accounts exist)
echo "3. Testing set_current_account tool..."
echo "   (This will fail if no accounts are configured - that's expected)"
curl -s -X POST "${API_BASE}/mcp/execute" \
  -H "X-API-Key: ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "set_current_account",
    "parameters": {
      "account_id": "test-account-id"
    }
  }' | jq '.'
echo ""

echo "====================================="
echo "Test complete!"
echo ""
echo "Note: To fully test these tools, you need to:"
echo "1. Start the rustymail server: ./target/release/rustymail-server"
echo "2. Configure at least one email account via the dashboard"
echo "3. Run this script again with a valid account_id"
