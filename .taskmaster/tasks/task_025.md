# Task ID: 25

**Title:** Add API-key and scope validation to all MCP endpoints

**Status:** done

**Dependencies:** 22 ✓, 23 ✓, 24 ✓

**Priority:** high

**Description:** Implement mandatory API-key validation middleware for all MCP routes in mcp_http.rs with per-tool scope requirements and proper error responses for authentication/authorization failures.

**Details:**

Modify src/api/mcp_http.rs to add comprehensive security to mcp_post_handler and mcp_get_handler which currently rely only on weak origin checks:

1) Create a new middleware module src/api/auth/api_key_middleware.rs with:
   - ApiKey struct containing key, scopes, and metadata
   - validate_api_key() function that checks against a database table or config file
   - extract_api_key_from_request() to get key from Authorization header (Bearer token) or X-API-Key header
   - ApiKeyMiddleware that intercepts all MCP requests before handlers

2) Define scope requirements for each MCP tool:
   - Low-level tools: email:read, email:write, folder:read, etc.
   - High-level tools: ai:execute, model:configure, email:draft
   - Create a tool_scopes mapping in high_level_tools.rs and regular tools module

3) Update mcp_post_handler and mcp_get_handler:
   - Remove or supplement weak origin check with API key validation
   - Extract requested tool from the JSON-RPC request
   - Look up required scopes for the tool
   - Validate API key has all required scopes
   - Pass validated API key context to downstream handlers

4) Implement proper error responses:
   - 401 Unauthorized for missing or invalid API keys
   - 403 Forbidden for valid key but insufficient scopes
   - Include WWW-Authenticate header with realm="MCP API"
   - Return JSON-RPC error format with descriptive messages

5) Create database schema for API keys:
   ```sql
   CREATE TABLE api_keys (
     id SERIAL PRIMARY KEY,
     key_hash VARCHAR(255) UNIQUE NOT NULL,
     name VARCHAR(255),
     scopes TEXT[], -- Array of scope strings
     created_at TIMESTAMP DEFAULT NOW(),
     last_used_at TIMESTAMP,
     is_active BOOLEAN DEFAULT true
   );
   ```

6) Add configuration for API key validation:
   - Environment variable to enable/disable in development
   - Option to load keys from config file for testing
   - Rate limiting per API key to prevent abuse

**Test Strategy:**

Test the API key validation thoroughly:

1) Unit tests for api_key_middleware.rs:
   - Test API key extraction from different header formats
   - Test scope validation logic with various scope combinations
   - Test database queries for API key lookup

2) Integration tests for MCP endpoints:
   - Test requests without API key return 401
   - Test requests with invalid API key return 401
   - Test requests with valid key but missing scopes return 403
   - Test successful requests with proper API key and scopes
   - Test both mcp_post_handler and mcp_get_handler paths

3) Test each tool's scope requirements:
   - Verify low-level tools require appropriate read/write scopes
   - Verify high-level AI tools require elevated scopes
   - Test scope inheritance (e.g., email:write includes email:read)

4) Security testing:
   - Attempt to bypass with malformed headers
   - Test SQL injection in API key lookup
   - Verify timing attacks don't reveal key existence
   - Test rate limiting prevents brute force

5) End-to-end testing:
   - Create test API keys with different scope sets
   - Verify frontend MCP client can authenticate properly
   - Test error handling in UI when authentication fails
   - Verify performance impact is minimal
