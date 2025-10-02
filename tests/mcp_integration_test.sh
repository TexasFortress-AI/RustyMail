#!/bin/bash

# MCP Streamable HTTP Integration Test Suite
# Tests compliance with MCP specification version 2025-03-26

set -e

echo "================================"
echo "MCP Integration Test Suite"
echo "================================"
echo ""

# Configuration
SERVER_URL="http://localhost:9437"
PASSED=0
FAILED=0

# Test helper functions
test_passed() {
    echo "✓ $1"
    ((PASSED++))
}

test_failed() {
    echo "✗ $1"
    echo "  Error: $2"
    ((FAILED++))
}

# Test 1: Server health check
echo "Test 1: Server Health Check"
if timeout 5 curl -s "${SERVER_URL}/health" > /dev/null 2>&1; then
    test_passed "Server is running"
else
    test_failed "Server is not running" "Cannot connect to ${SERVER_URL}"
    exit 1
fi
echo ""

# Test 2: Initialize method
echo "Test 2: Initialize Method"
INIT_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')

if echo "$INIT_RESPONSE" | jq -e '.result.protocolVersion == "2025-03-26"' > /dev/null; then
    test_passed "Protocol version is 2025-03-26"
else
    test_failed "Protocol version check" "Expected 2025-03-26"
fi

if echo "$INIT_RESPONSE" | jq -e '.result.serverInfo.name == "rustymail-mcp"' > /dev/null; then
    test_passed "Server name is correct"
else
    test_failed "Server name check" "Expected rustymail-mcp"
fi

SESSION_ID=$(echo "$INIT_RESPONSE" | jq -r '.result._meta.sessionId')
if [ -n "$SESSION_ID" ] && [ "$SESSION_ID" != "null" ]; then
    test_passed "Session ID generated: $SESSION_ID"
else
    test_failed "Session ID generation" "No session ID in response"
fi
echo ""

# Test 3: Tools list method
echo "Test 3: Tools List Method"
TOOLS_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}')

TOOL_COUNT=$(echo "$TOOLS_RESPONSE" | jq '.result.tools | length')
if [ "$TOOL_COUNT" -ge 3 ]; then
    test_passed "Tools list returned $TOOL_COUNT tools"
else
    test_failed "Tools list" "Expected at least 3 tools, got $TOOL_COUNT"
fi

if echo "$TOOLS_RESPONSE" | jq -e '.result.tools[] | select(.name == "list_folders")' > /dev/null; then
    test_passed "list_folders tool exists"
else
    test_failed "list_folders tool" "Tool not found"
fi
echo ""

# Test 4: SSE streaming (with timeout)
echo "Test 4: SSE Streaming"
SSE_OUTPUT=$(timeout 3 curl -N -H "Accept: text/event-stream" "${SERVER_URL}/mcp" 2>&1 || true)

if echo "$SSE_OUTPUT" | grep -q "connected"; then
    test_passed "SSE connection established"
else
    test_failed "SSE connection" "No connection message received"
fi

if echo "$SSE_OUTPUT" | grep -q "heartbeat"; then
    test_passed "SSE heartbeat received"
else
    test_failed "SSE heartbeat" "No heartbeat message received"
fi
echo ""

# Test 5: Session management
echo "Test 5: Session Management"
# First request to create session
RESPONSE1=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' -i)

SESSION_ID=$(echo "$RESPONSE1" | grep -i "Mcp-Session-Id:" | cut -d' ' -f2 | tr -d '\r\n')

if [ -n "$SESSION_ID" ]; then
    test_passed "Session ID received in header: $SESSION_ID"

    # Second request using session ID
    RESPONSE2=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
        -H 'Content-Type: application/json' \
        -H 'Accept: application/json' \
        -H "Mcp-Session-Id: $SESSION_ID" \
        -d '{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}')

    if echo "$RESPONSE2" | jq -e '.result.tools' > /dev/null; then
        test_passed "Session reused successfully"
    else
        test_failed "Session reuse" "Request with session ID failed"
    fi
else
    test_failed "Session ID in header" "No Mcp-Session-Id header found"
fi
echo ""

# Test 6: Origin header validation
echo "Test 6: Origin Header Validation"
# Test with allowed origin (localhost)
RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -H 'Origin: http://localhost:3000' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' -w "%{http_code}")

if echo "$RESPONSE" | grep -q "200"; then
    test_passed "Localhost origin accepted"
else
    test_failed "Origin validation" "Localhost origin rejected"
fi
echo ""

# Test 7: Error handling
echo "Test 7: Error Handling"
ERROR_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":99,"method":"invalid_method","params":{}}')

if echo "$ERROR_RESPONSE" | jq -e '.error.code == -32601' > /dev/null; then
    test_passed "Method not found error returned"
else
    test_failed "Error handling" "Expected error code -32601"
fi
echo ""

# Test 8: Versioned endpoint
echo "Test 8: Versioned Endpoint"
V1_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp/v1" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')

if echo "$V1_RESPONSE" | jq -e '.result.serverInfo' > /dev/null; then
    test_passed "/mcp/v1 endpoint works"
else
    test_failed "/mcp/v1 endpoint" "Versioned endpoint not responding"
fi
echo ""

# Test 9: Accept header handling
echo "Test 9: Accept Header Handling"
# Test JSON response
JSON_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    -i | grep -i "Content-Type:")

if echo "$JSON_RESPONSE" | grep -q "application/json"; then
    test_passed "JSON content type returned for application/json accept"
else
    test_failed "Accept header JSON" "Expected application/json content type"
fi

# Test SSE response format
SSE_RESPONSE=$(timeout 5 curl -s -X POST "${SERVER_URL}/mcp" \
    -H 'Content-Type: application/json' \
    -H 'Accept: text/event-stream' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    -i | grep -i "Content-Type:")

if echo "$SSE_RESPONSE" | grep -q "text/event-stream"; then
    test_passed "SSE content type returned for text/event-stream accept"
else
    test_failed "Accept header SSE" "Expected text/event-stream content type"
fi
echo ""

# Test 10: GET method validation
echo "Test 10: GET Method Validation"
# GET without proper Accept header should fail
GET_NO_SSE=$(timeout 5 curl -s -o /dev/null -w "%{http_code}" -X GET "${SERVER_URL}/mcp")

if [ "$GET_NO_SSE" = "405" ]; then
    test_passed "GET without SSE accept header rejected (405)"
else
    test_failed "GET method validation" "Expected 405, got $GET_NO_SSE"
fi
echo ""

# Summary
echo "================================"
echo "Test Summary"
echo "================================"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "✓ All tests passed!"
    exit 0
else
    echo "✗ Some tests failed"
    exit 1
fi
