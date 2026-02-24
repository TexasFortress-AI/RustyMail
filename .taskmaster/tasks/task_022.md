# Task ID: 22

**Title:** Fix permissive CORS configuration in main.rs

**Status:** done

**Dependencies:** 32 ✓

**Priority:** high

**Description:** Replace the permissive CORS configuration that allows any origin, method, and header with a secure whitelist-based approach using environment variables to prevent CSRF attacks.

**Details:**

Update the CORS configuration in src/main.rs (lines 270-277) to implement a secure origin whitelist:

1) Add a new environment variable ALLOWED_ORIGINS that accepts a comma-separated list of allowed origins (e.g., "http://localhost:3000,https://dashboard.example.com")

2) Replace the current permissive configuration:
   ```rust
   Cors::default()
       .allow_any_origin()
       .allow_any_method()
       .allow_any_header()
   ```

3) With a secure configuration:
   ```rust
   let allowed_origins = std::env::var("ALLOWED_ORIGINS")
       .unwrap_or_else(|_| "http://localhost:3000".to_string())
       .split(',')
       .map(|s| s.trim().to_string())
       .collect::<Vec<String>>();
   
   let cors = Cors::default()
       .allowed_origins(
           allowed_origins
               .iter()
               .map(|origin| origin.parse::<HeaderValue>().unwrap())
               .collect::<Vec<_>>()
               .as_slice()
       )
       .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
       .allowed_headers(vec![
           header::CONTENT_TYPE,
           header::AUTHORIZATION,
           header::ACCEPT,
       ])
       .supports_credentials()
       .max_age(3600);
   ```

4) Update the .env.example file to include the new ALLOWED_ORIGINS variable with sensible defaults

5) Add validation to ensure at least one origin is configured and that origins are valid URLs

6) Consider adding a warning log if ALLOWED_ORIGINS is not set, defaulting to localhost only for development safety

7) Ensure the CORS middleware properly handles preflight OPTIONS requests

8) Update any deployment documentation to specify the ALLOWED_ORIGINS configuration requirement

**Test Strategy:**

Verify the CORS fix by:

1) Start the server without ALLOWED_ORIGINS set and confirm it defaults to localhost:3000 only
2) Set ALLOWED_ORIGINS="http://localhost:3000,http://localhost:5173" and restart the server
3) Test that requests from allowed origins work correctly:
   - Make API calls from http://localhost:3000 and verify they succeed
   - Make API calls from http://localhost:5173 and verify they succeed
4) Test that requests from non-allowed origins are blocked:
   - Use curl or a browser from http://localhost:8080 and verify CORS error
   - Try making requests from https://evil.com and confirm they're rejected
5) Verify preflight OPTIONS requests are handled correctly for allowed origins
6) Test with credentials (cookies/auth headers) to ensure supports_credentials() works
7) Check server logs for appropriate warnings when ALLOWED_ORIGINS is not configured
8) Verify that malformed origins in ALLOWED_ORIGINS cause server startup to fail with clear error message
