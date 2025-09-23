#!/bin/bash
# Test the configuration service API endpoints

echo "Testing Configuration Service API..."

# Start the server in background (if not already running)
# cargo run --bin rustymail-server &
# SERVER_PID=$!
# sleep 5

BASE_URL="http://localhost:3001/api/dashboard"

# Test 1: Get current configuration
echo "Test 1: Get current configuration..."
curl -X GET "$BASE_URL/config" -H "Content-Type: application/json" | jq '.' || echo "Failed to get config"

# Test 2: Validate configuration
echo -e "\nTest 2: Validate configuration..."
curl -X GET "$BASE_URL/config/validate" -H "Content-Type: application/json" | jq '.' || echo "Failed to validate config"

# Test 3: Update IMAP configuration
echo -e "\nTest 3: Update IMAP configuration..."
curl -X PUT "$BASE_URL/config/imap" \
  -H "Content-Type: application/json" \
  -d '{
    "host": "imap.gmail.com",
    "port": 993,
    "user": "test@gmail.com",
    "pass": "testpass123"
  }' | jq '.' || echo "Failed to update IMAP config"

# Test 4: Update REST configuration
echo -e "\nTest 4: Update REST configuration..."
curl -X PUT "$BASE_URL/config/rest" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": true,
    "host": "0.0.0.0",
    "port": 8080
  }' | jq '.' || echo "Failed to update REST config"

# Test 5: Update Dashboard configuration
echo -e "\nTest 5: Update Dashboard configuration..."
curl -X PUT "$BASE_URL/config/dashboard" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": true,
    "port": 3001,
    "path": "/tmp"
  }' | jq '.' || echo "Failed to update Dashboard config"

# Test 6: Get updated configuration
echo -e "\nTest 6: Get updated configuration..."
curl -X GET "$BASE_URL/config" -H "Content-Type: application/json" | jq '.' || echo "Failed to get updated config"

# Clean up
# kill $SERVER_PID 2>/dev/null

echo -e "\nConfiguration Service API tests completed!"