# Task ID: 23

**Title:** Fix origin validation bypass in MCP HTTP backend

**Status:** done

**Dependencies:** 22 ✓

**Priority:** high

**Description:** Fix critical security vulnerability in src/api/mcp_http.rs (lines 171-189) where origin validation accepts any domain containing 'localhost' and allows requests with no Origin header, enabling CSRF attacks.

**Details:**

Update the origin validation logic in src/api/mcp_http.rs to implement secure origin checking:

1) **Fix substring matching vulnerability** (line ~175-180):
   - Replace the current logic that accepts any origin containing 'localhost' (e.g., evil.localhost.com)
   - Implement exact string matching for allowed origins
   - Use a whitelist approach with full origin strings including protocol and port

2) **Require Origin header on all requests**:
   - Remove the logic that allows requests with missing Origin headers
   - Return 403 Forbidden for requests without Origin header
   - Add proper error message: "Origin header required"

3) **Implement secure origin validation**:
   ```rust
   // Add at top of file
   use std::env;
   
   // In the origin validation section
   let allowed_origins = env::var("ALLOWED_MCP_ORIGINS")
       .unwrap_or_else(|_| "http://localhost:3000".to_string())
       .split(',')
       .map(|s| s.trim().to_string())
       .collect::<Vec<String>>();
   
   // Validate origin
   if let Some(origin) = req.headers().get("Origin") {
       let origin_str = origin.to_str().unwrap_or("");
       if !allowed_origins.contains(&origin_str.to_string()) {
           return Ok(Response::builder()
               .status(StatusCode::FORBIDDEN)
               .body(Body::from("Origin not allowed"))
               .unwrap());
       }
   } else {
       return Ok(Response::builder()
           .status(StatusCode::FORBIDDEN)
           .body(Body::from("Origin header required"))
           .unwrap());
   }
   ```

4) **Add environment variable configuration**:
   - Support ALLOWED_MCP_ORIGINS environment variable
   - Accept comma-separated list of full origins (e.g., "http://localhost:3000,http://localhost:5173,https://app.example.com")
   - Default to "http://localhost:3000" if not set

5) **Update CORS headers in response**:
   - Set Access-Control-Allow-Origin to the specific requesting origin (not "*")
   - Only set it if the origin is in the allowed list

6) **Consider preflight requests**:
   - Ensure OPTIONS requests also validate origins
   - Return appropriate CORS headers only for allowed origins

**Test Strategy:**

Verify the security fix with comprehensive testing:

1) **Test substring matching fix**:
   - Send request with Origin: http://evil.localhost.com - should be rejected (403)
   - Send request with Origin: http://localhost.attacker.com - should be rejected (403)
   - Send request with Origin: http://localhost:3000 - should be allowed (200)

2) **Test missing Origin header enforcement**:
   - Use curl without Origin header: `curl http://localhost:8080/mcp/tools/list` - should be rejected (403)
   - Use curl with valid Origin: `curl -H "Origin: http://localhost:3000" http://localhost:8080/mcp/tools/list` - should work

3) **Test environment variable configuration**:
   - Set ALLOWED_MCP_ORIGINS="http://localhost:3000,http://localhost:5173"
   - Verify requests from localhost:3000 work
   - Verify requests from localhost:5173 work
   - Verify requests from localhost:8080 are rejected

4) **Test exact matching with ports**:
   - Origin: http://localhost:3000 with ALLOWED_MCP_ORIGINS="http://localhost:3001" - should fail
   - Origin: http://localhost:3001 with ALLOWED_MCP_ORIGINS="http://localhost:3001" - should work

5) **Test CORS response headers**:
   - Verify Access-Control-Allow-Origin is set to the specific origin (not "*")
   - Verify it's only set when origin is allowed

6) **Test preflight OPTIONS requests**:
   - Send OPTIONS request with valid origin - should return proper CORS headers
   - Send OPTIONS request with invalid origin - should be rejected
