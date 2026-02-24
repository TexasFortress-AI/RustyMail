# Task ID: 28

**Title:** Wire rate limiting into REST and MCP API paths

**Status:** done

**Dependencies:** 22 ✓

**Priority:** medium

**Description:** Integrate the existing rate limiting validation from validation.rs into all REST API endpoints and MCP handlers, implementing per-IP and per-API-key limits with proper 429 responses and rate limit headers.

**Details:**

Implement comprehensive rate limiting across all API surfaces:

1) **Create rate limiting middleware for REST APIs**:
   - Create src/dashboard/middleware/rate_limit.rs
   - Import existing rate limiting logic from validation.rs
   - Implement RateLimitMiddleware that extracts client IP and API key from requests
   - Use a token bucket or sliding window algorithm for tracking request counts
   - Store rate limit state in memory with Arc<Mutex<HashMap>> or use Redis for distributed deployments

2) **Configure rate limits via environment variables**:
   - Add RATE_LIMIT_PER_MINUTE (default: 60)
   - Add RATE_LIMIT_PER_HOUR (default: 1000)
   - Add RATE_LIMIT_PER_IP_MINUTE (default: 30)
   - Add RATE_LIMIT_PER_IP_HOUR (default: 500)
   - Support different limits for authenticated (API key) vs anonymous requests

3) **Integrate middleware into REST API routes**:
   - In main.rs, wrap all API routes with rate limiting middleware
   - Apply before authentication middleware to protect against auth bypass attempts
   - Example:
   ```rust
   .wrap(RateLimitMiddleware::new(rate_limit_config))
   .wrap(cors)
   .wrap(Logger::default())
   ```

4) **Add rate limiting to MCP handlers**:
   - In each MCP handler function, add rate limit check at the beginning
   - Extract client identifier from MCP context (connection ID or client metadata)
   - Use the same rate limiting logic but with MCP-specific limits
   - Return appropriate MCP error response when rate limited

5) **Implement 429 Too Many Requests responses**:
   - For REST APIs: Return HTTP 429 status with JSON error body
   - Include retry-after header indicating when client can retry
   - Error response format:
   ```json
   {
     "error": "rate_limit_exceeded",
     "message": "Too many requests. Please retry after 60 seconds.",
     "retry_after": 60
   }
   ```

6) **Add rate limit headers to all responses**:
   - X-RateLimit-Limit: Maximum requests allowed
   - X-RateLimit-Remaining: Requests remaining in current window
   - X-RateLimit-Reset: Unix timestamp when the rate limit resets
   - Add these headers even for successful requests

7) **Handle edge cases**:
   - Properly extract real client IP behind proxies (X-Forwarded-For, X-Real-IP)
   - Implement IP whitelist for internal services (via RATE_LIMIT_WHITELIST_IPS env var)
   - Graceful degradation if rate limit storage fails
   - Different rate limits for different API endpoints (e.g., higher for read, lower for write)

**Test Strategy:**

Verify rate limiting implementation with comprehensive testing:

1) **Unit tests for rate limiting logic**:
   - Test token bucket/sliding window algorithm correctness
   - Verify per-minute and per-hour limits work independently
   - Test IP-based vs API-key-based rate limiting
   - Verify rate limit reset timing

2) **Integration tests for REST API**:
   - Send requests up to the limit and verify all succeed
   - Send one more request and verify 429 response with correct headers
   - Wait for rate limit reset and verify requests work again
   - Test with different IPs and API keys to ensure isolation

3) **MCP handler rate limiting tests**:
   - Mock MCP requests and verify rate limiting applies
   - Test that rate limited MCP calls return appropriate error responses
   - Verify MCP rate limits are independent from REST API limits

4) **Header validation tests**:
   - Verify all responses include X-RateLimit-* headers
   - Check header values decrease correctly with each request
   - Verify Reset header contains valid future timestamp

5) **Load testing**:
   - Use Apache Bench or similar to send concurrent requests
   - Verify rate limiting holds under high concurrency
   - Test with multiple IPs to ensure no cross-contamination

6) **Configuration tests**:
   - Start server with custom rate limit env vars
   - Verify limits match configured values
   - Test with missing env vars to ensure defaults work

7) **Security tests**:
   - Attempt to bypass with spoofed headers
   - Verify whitelisted IPs bypass rate limits
   - Test rate limiting works before authentication
