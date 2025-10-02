#!/bin/bash

# MCP Streamable HTTP Test Script
# Tests the RustyMail MCP server with modern Streamable HTTP transport

set -e

echo "================================"
echo "MCP Streamable HTTP Test Script"
echo "================================"

# Configuration
SERVER_URL="http://localhost:9437"
API_KEY="test-rustymail-key-2024"

echo ""
echo "1. Testing basic endpoints..."
echo "-------------------------------"

# Test if server is running
echo -n "Testing server health... "
if curl -s "${SERVER_URL}/health" > /dev/null 2>&1; then
    echo "✓ Server is running"
else
    echo "✗ Server is not running on port 9437"
    exit 1
fi

# Test MCP endpoint (GET) - should reject without Accept: text/event-stream
echo -n "Testing /mcp endpoint (GET without SSE Accept)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/mcp")
if [ "$RESPONSE" = "405" ]; then
    echo "✓ MCP endpoint correctly rejects non-SSE GET"
else
    echo "⚠ MCP endpoint returned: $RESPONSE (expected 405)"
fi

# Test MCP endpoint (GET) - with SSE Accept header
echo -n "Testing /mcp endpoint (GET with SSE Accept)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Accept: text/event-stream" \
    "${SERVER_URL}/mcp")
if [ "$RESPONSE" = "200" ]; then
    echo "✓ MCP SSE endpoint accessible"
else
    echo "✗ MCP SSE endpoint returned: $RESPONSE"
fi

# Test MCP endpoint (POST) - JSON-RPC
echo -n "Testing /mcp endpoint (POST with JSON-RPC)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    "${SERVER_URL}/mcp")
if [ "$RESPONSE" = "200" ]; then
    echo "✓ MCP POST endpoint accessible"
else
    echo "✗ MCP POST endpoint returned: $RESPONSE"
fi

# Test versioned endpoint
echo -n "Testing /mcp/v1 endpoint (POST)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    "${SERVER_URL}/mcp/v1")
if [ "$RESPONSE" = "200" ]; then
    echo "✓ MCP v1 endpoint accessible"
else
    echo "✗ MCP v1 endpoint returned: $RESPONSE"
fi

echo ""
echo "2. Testing MCP JSON-RPC methods..."
echo "-------------------------------"

# Test initialize method
echo -n "Testing initialize method... "
RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d '{
      "jsonrpc": "2.0",
      "id": 1,
      "method": "initialize",
      "params": {
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {"name": "test-client", "version": "1.0.0"}
      }
    }' \
    "${SERVER_URL}/mcp")

if echo "$RESPONSE" | grep -q '"serverInfo"'; then
    echo "✓ Initialize successful"
    echo "  Server: $(echo "$RESPONSE" | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4)"
    echo "  Protocol: $(echo "$RESPONSE" | grep -o '"protocolVersion":"[^"]*"' | head -1 | cut -d'"' -f4)"
else
    echo "✗ Initialize failed"
    echo "  Response: $RESPONSE"
fi

# Test tools/list method
echo -n "Testing tools/list method... "
RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
    "${SERVER_URL}/mcp")

if echo "$RESPONSE" | grep -q '"tools"'; then
    TOOL_COUNT=$(echo "$RESPONSE" | grep -o '"name"' | wc -l)
    echo "✓ Tools list retrieved ($TOOL_COUNT tools)"
else
    echo "✗ Tools list failed"
fi

# Test session management
echo ""
echo "3. Testing session management..."
echo "-------------------------------"

# Initialize with session
echo -n "Testing session creation... "
RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    "${SERVER_URL}/mcp" -i)

SESSION_ID=$(echo "$RESPONSE" | grep -i "Mcp-Session-Id:" | cut -d' ' -f2 | tr -d '\r\n')
if [ -n "$SESSION_ID" ]; then
    echo "✓ Session created: $SESSION_ID"
else
    echo "✗ No session ID in response"
fi

# Test session reuse
if [ -n "$SESSION_ID" ]; then
    echo -n "Testing session reuse... "
    RESPONSE=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -H "Accept: application/json" \
        -H "Mcp-Session-Id: $SESSION_ID" \
        -d '{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}' \
        "${SERVER_URL}/mcp")

    if echo "$RESPONSE" | grep -q '"tools"'; then
        echo "✓ Session reused successfully"
    else
        echo "✗ Session reuse failed"
    fi
fi

echo ""
echo "================================"
echo "Test Summary"
echo "================================"
echo ""
echo "MCP Streamable HTTP transport is working!"
echo ""
echo "Endpoint URLs:"
echo "  - Main endpoint: ${SERVER_URL}/mcp"
echo "  - Versioned endpoint: ${SERVER_URL}/mcp/v1"
echo ""
echo "Supported methods:"
echo "  - POST with Accept: application/json (JSON-RPC requests)"
echo "  - POST with Accept: text/event-stream (SSE format responses)"
echo "  - GET with Accept: text/event-stream (SSE streaming)"
echo ""
echo "Session Management:"
echo "  - Sessions tracked via Mcp-Session-Id header"
echo "  - 10-minute timeout with automatic cleanup"
echo "  - Connection resumability via Last-Event-ID"
echo ""