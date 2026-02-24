# Task ID: 32

**Title:** Add comprehensive test coverage for security-affected areas

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Create extensive test suite covering all security-critical functionality including CORS, origin validation, API authentication, path traversal, and rate limiting before implementing security fixes.

**Details:**

Create a comprehensive security test suite in tests/integration/security_tests.rs that establishes baseline behavior for all security-critical areas. This must be completed before any security hardening begins.

**1. CORS Configuration Tests (for Task 22):**
```rust
#[cfg(test)]
mod cors_tests {
    use actix_web::{test, App, http::header};
    
    #[actix_web::test]
    async fn test_cors_blocks_unauthorized_origins() {
        // Test that requests from non-whitelisted origins are blocked
        let app = test::init_service(create_app()).await;
        let req = test::TestRequest::get()
            .uri("/api/emails")
            .header(header::ORIGIN, "https://evil.com")
            .to_request();
        let resp = test::call_service(&app, req).await;
        // Initially may pass (document current behavior)
    }
    
    #[actix_web::test]
    async fn test_cors_allows_configured_origins() {
        // Test that ALLOWED_ORIGINS are properly accepted
        std::env::set_var("ALLOWED_ORIGINS", "http://localhost:3000,http://localhost:5173");
        // Test requests from these origins succeed
    }
    
    #[actix_web::test]
    async fn test_preflight_options_requests() {
        // Test OPTIONS preflight requests work correctly
        // Check Access-Control headers in response
    }
    
    #[actix_web::test]
    async fn test_cors_credentials_mode() {
        // Test Access-Control-Allow-Credentials header
    }
}
```

**2. Origin Validation Tests (for Task 23):**
```rust
mod origin_validation_tests {
    #[actix_web::test]
    async fn test_exact_origin_matching() {
        // Test that "evil.localhost.com" doesn't match "localhost.com"
        // Test substring matching is rejected
    }
    
    #[actix_web::test]
    async fn test_missing_origin_header() {
        // Test requests without Origin header
        // Document current behavior (may currently pass)
    }
    
    #[actix_web::test]
    async fn test_spoofed_origin_patterns() {
        // Test various spoofing attempts:
        // - "localhost.evil.com"
        // - "localhost.com.evil.com"
        // - "localhost:3000.evil.com"
    }
    
    #[actix_web::test]
    async fn test_port_number_validation() {
        // Test that port numbers are validated
        // "localhost:3000" != "localhost:3001"
    }
}
```

**3. API Key Authentication Tests (for Tasks 24, 25):**
```rust
mod api_auth_tests {
    #[actix_web::test]
    async fn test_mcp_endpoints_require_api_key() {
        // Test all MCP endpoints reject requests without API key
        let endpoints = vec![
            "/mcp/tools",
            "/mcp/tools/list_emails/run",
            "/mcp/tools/get_email/run",
            // ... all MCP endpoints
        ];
        
        for endpoint in endpoints {
            let req = test::TestRequest::post()
                .uri(endpoint)
                .to_request();
            let resp = test::call_service(&app, req).await;
            // Document current behavior
        }
    }
    
    #[actix_web::test]
    async fn test_api_key_scope_enforcement() {
        // Test that API keys with limited scopes are properly restricted
        // Test read-only keys can't write
        // Test MCP-only keys can't access REST endpoints
    }
    
    #[actix_web::test]
    async fn test_invalid_api_key_rejection() {
        // Test expired keys, malformed keys, non-existent keys
    }
    
    #[actix_web::test]
    async fn test_no_test_credentials_seeded() {
        // Verify database doesn't contain default test API keys
        // Check for common test patterns: "test", "demo", "example"
    }
}
```

**4. Path Traversal Tests (for Task 27):**
```rust
mod path_traversal_tests {
    #[actix_web::test]
    async fn test_directory_traversal_patterns() {
        // Test various traversal attempts:
        let patterns = vec![
            "../../../etc/passwd",
            "..\\..\\windows\\system32",
            "%2e%2e%2f",
            "..%252f",
            "%c0%ae%c0%ae/",
            "....//",
            "..;/",
        ];
        
        for pattern in patterns {
            // Test attachment download/upload with malicious paths
            // Document current behavior
        }
    }
    
    #[actix_web::test]
    async fn test_symlink_escape_attempts() {
        // Create symlink pointing outside storage directory
        // Test that following symlinks is blocked
    }
    
    #[actix_web::test]
    async fn test_path_canonicalization() {
        // Test that paths are properly canonicalized
        // Test relative paths are resolved
    }
    
    #[actix_web::test]
    async fn test_storage_directory_containment() {
        // Verify all file operations stay within designated directory
    }
}
```

**5. Rate Limiting Tests (for Task 28):**
```rust
mod rate_limiting_tests {
    #[actix_web::test]
    async fn test_rest_api_rate_limits() {
        // Test rate limiting on REST endpoints
        // Make requests exceeding limit
        // Verify 429 response
    }
    
    #[actix_web::test]
    async fn test_mcp_api_rate_limits() {
        // Test rate limiting on MCP endpoints
        // Ensure MCP routes are also protected
    }
    
    #[actix_web::test]
    async fn test_rate_limit_headers() {
        // Check for X-RateLimit-Limit header
        // Check for X-RateLimit-Remaining header
        // Check for X-RateLimit-Reset header
    }
    
    #[actix_web::test]
    async fn test_rate_limit_429_response() {
        // Verify proper 429 Too Many Requests response
        // Check Retry-After header
    }
}
```

**Implementation Guidelines:**
- Each test should first document CURRENT behavior (even if insecure)
- Add clear comments indicating expected vs actual behavior
- Tests should be designed to pass with current code
- As security fixes are implemented, update tests to verify secure behavior
- Use test fixtures and helper functions to reduce duplication
- Include both positive and negative test cases
- Test edge cases and boundary conditions

**Test Strategy:**

Verify comprehensive test coverage implementation:

1. **Test File Creation:**
   - Confirm tests/integration/security_tests.rs is created
   - Verify all five test modules are present
   - Check that tests compile without errors

2. **CORS Tests Validation:**
   - Run CORS tests and document current permissive behavior
   - Verify tests check origin validation, preflight, and credentials
   - Confirm tests are ready to validate Task 22 fixes

3. **Origin Validation Tests:**
   - Run origin tests documenting current behavior
   - Verify exact matching tests (not substring)
   - Confirm spoofing patterns are tested

4. **API Authentication Tests:**
   - Run auth tests on all MCP endpoints
   - Document which endpoints currently lack authentication
   - Verify scope enforcement tests are present

5. **Path Traversal Tests:**
   - Run traversal tests with various attack patterns
   - Document current vulnerability status
   - Verify symlink and canonicalization tests work

6. **Rate Limiting Tests:**
   - Run rate limit tests on both REST and MCP routes
   - Document current rate limiting status
   - Verify 429 response and header tests

7. **Test Execution:**
   ```bash
   cargo test --test security_tests -- --nocapture
   ```
   - All tests should run (may fail documenting insecure behavior)
   - Generate test report showing coverage gaps

8. **Documentation Review:**
   - Each test should have clear comments
   - Current vs expected behavior documented
   - Ready for updates as security fixes are applied
