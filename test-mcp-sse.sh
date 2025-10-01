#!/bin/bash

# MCP SSE Test Script using supergateway
# Tests the RustyMail MCP server with proper endpoints

set -e

echo "================================"
echo "MCP SSE Test Script"
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

# Test SSE endpoint (GET)
echo -n "Testing /sse endpoint (GET)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -H "X-API-Key: ${API_KEY}" "${SERVER_URL}/sse")
if [ "$RESPONSE" = "200" ]; then
    echo "✓ SSE endpoint accessible"
else
    echo "✗ SSE endpoint returned: $RESPONSE"
fi

# Test message endpoint (POST)
echo -n "Testing /message endpoint (POST)... "
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "X-API-Key: ${API_KEY}" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"test"}' \
    "${SERVER_URL}/message")
if [ "$RESPONSE" = "200" ]; then
    echo "✓ Message endpoint accessible"
else
    echo "✗ Message endpoint returned: $RESPONSE"
fi

echo ""
echo "2. Testing MCP with supergateway..."
echo "-------------------------------"

# Create a test file for initialize request
cat > /tmp/mcp_test_init.json << 'EOF'
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": {},
    "clientInfo": {
      "name": "test-client",
      "version": "1.0.0"
    }
  }
}
EOF

echo "Starting supergateway test..."
echo ""

# Run supergateway with timeout and capture output
timeout 5 npx -y supergateway \
    --sse "${SERVER_URL}" \
    --header "X-API-Key: ${API_KEY}" \
    --logLevel info \
    < /tmp/mcp_test_init.json \
    2>&1 | tee /tmp/supergateway_test.log &

SG_PID=$!

# Wait a bit for connection
sleep 2

# Check if supergateway is still running
if ps -p $SG_PID > /dev/null 2>&1; then
    echo "✓ Supergateway connected successfully"

    # Kill it after test
    kill $SG_PID 2>/dev/null || true
    wait $SG_PID 2>/dev/null || true
else
    echo "✗ Supergateway failed to connect"
    echo "Output:"
    cat /tmp/supergateway_test.log
fi

echo ""
echo "3. Testing direct initialization..."
echo "-------------------------------"

# Test sending initialize through supergateway pipe
echo "Testing initialize method..."

# Create a named pipe
PIPE_FILE="/tmp/mcp_test_pipe_$$"
mkfifo "$PIPE_FILE"

# Start supergateway in background with pipe
npx -y supergateway \
    --sse "${SERVER_URL}" \
    --header "X-API-Key: ${API_KEY}" \
    --logLevel debug \
    < "$PIPE_FILE" \
    > /tmp/mcp_response.log 2>&1 &

SG_PID=$!

# Send initialize request
cat /tmp/mcp_test_init.json > "$PIPE_FILE" &

# Wait for response
sleep 3

# Check response
if grep -q '"serverInfo"' /tmp/mcp_response.log 2>/dev/null; then
    echo "✓ Initialize response received"
    echo "Response contains:"
    grep -o '"name":"[^"]*"' /tmp/mcp_response.log | head -1
    grep -o '"protocolVersion":"[^"]*"' /tmp/mcp_response.log | head -1
else
    echo "✗ No valid initialize response"
    echo "Log output:"
    tail -20 /tmp/mcp_response.log
fi

# Cleanup
kill $SG_PID 2>/dev/null || true
rm -f "$PIPE_FILE" /tmp/mcp_test_init.json

echo ""
echo "================================"
echo "Test Summary"
echo "================================"
echo ""
echo "Configuration for Claude Desktop should be:"
echo ""
echo '  "rustymail-sse": {'
echo '    "command": "npx",'
echo '    "args": ['
echo '      "-y",'
echo '      "supergateway",'
echo '      "--sse",'
echo '      "http://localhost:9437",'
echo '      "--header",'
echo '      "X-API-Key: test-rustymail-key-2024"'
echo '    ]'
echo '  }'
echo ""