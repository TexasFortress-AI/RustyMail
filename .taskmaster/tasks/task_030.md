# Task ID: 30

**Title:** Reduce unwrap/expect usage in request handlers

**Status:** done

**Dependencies:** 25 ✓

**Priority:** low

**Description:** Replace 599 unwrap/expect calls in request handlers with proper Result handling and error responses, focusing on handlers that process external input to prevent panics from unexpected data.

**Details:**

Systematically eliminate panic-inducing unwrap/expect calls from request handlers to improve application stability and security:

1) **Audit and prioritize unwrap/expect usage**:
   - Run `rg -c "\.unwrap\(\)|\.expect\(" src/` to get current count and locations
   - Focus on high-risk areas: src/api/, src/dashboard/handlers/, and MCP handlers
   - Prioritize handlers that process external input: API endpoints, form submissions, file uploads
   - Create a tracking spreadsheet with file, line number, risk level, and replacement strategy

2) **Define proper error types**:
   - Create src/errors/handler_errors.rs with custom error types:
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum HandlerError {
       #[error("Invalid input: {0}")]
       InvalidInput(String),
       #[error("Database error: {0}")]
       Database(#[from] sqlx::Error),
       #[error("Serialization error: {0}")]
       Serialization(#[from] serde_json::Error),
       #[error("IO error: {0}")]
       Io(#[from] std::io::Error),
       #[error("Authentication failed")]
       Unauthorized,
       #[error("Resource not found")]
       NotFound,
   }
   ```
   - Implement ResponseError trait for automatic HTTP response conversion

3) **Replace unwrap/expect in API handlers**:
   - Convert unwrap() to ? operator where possible
   - Replace expect() with map_err() to provide context:
   ```rust
   // Before
   let user_id = req.param("id").unwrap().parse::<i32>().unwrap();
   
   // After
   let user_id = req.param("id")
       .ok_or(HandlerError::InvalidInput("Missing user ID".into()))?
       .parse::<i32>()
       .map_err(|_| HandlerError::InvalidInput("Invalid user ID format".into()))?;
   ```

4) **Handle JSON parsing safely**:
   - Replace serde_json::from_str().unwrap() with proper error handling:
   ```rust
   // Before
   let config: Config = serde_json::from_str(&body).unwrap();
   
   // After
   let config: Config = serde_json::from_str(&body)
       .map_err(|e| HandlerError::InvalidInput(format!("Invalid JSON: {}", e)))?;
   ```

5) **Fix database query handling**:
   - Replace query unwraps with proper Result propagation:
   ```rust
   // Before
   let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
       .fetch_one(&pool)
       .await
       .unwrap();
   
   // After
   let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
       .fetch_one(&pool)
       .await
       .map_err(|e| match e {
           sqlx::Error::RowNotFound => HandlerError::NotFound,
           _ => HandlerError::Database(e),
       })?;
   ```

6) **Update MCP handlers**:
   - Focus on mcp_http.rs handlers that process tool calls
   - Replace unwrap in JSON-RPC parsing and response building
   - Add proper error responses following JSON-RPC error format

7) **Implement error response middleware**:
   - Create middleware to convert HandlerError to appropriate HTTP responses
   - Include error details in development, sanitized messages in production
   - Add request ID for error tracking

**Test Strategy:**

Verify the unwrap/expect reduction with comprehensive testing:

1) **Static analysis verification**:
   - Run `rg -c "\.unwrap\(\)|\.expect\(" src/api/ src/dashboard/handlers/` before and after
   - Verify significant reduction in count (target: 80%+ reduction in these directories)
   - Use clippy with `#![warn(clippy::unwrap_used, clippy::expect_used)]` on modified files

2) **Unit tests for error handling**:
   - Test each HandlerError variant converts to correct HTTP status code
   - Verify error messages are properly formatted and sanitized
   - Test error context preservation through map_err chains

3) **Integration tests for API endpoints**:
   - Send malformed JSON to endpoints, verify 400 Bad Request responses
   - Test with invalid IDs, verify 404 Not Found responses
   - Send requests missing required fields, verify descriptive error messages
   - Test database connection failures return 500 Internal Server Error

4) **Panic testing**:
   - Set up panic hook to log and alert on any remaining panics
   - Run fuzzing tests on API endpoints with random/malformed input
   - Monitor application logs during testing for any panic messages

5) **Load testing for stability**:
   - Run load tests with mix of valid and invalid requests
   - Verify no panics occur under high load with bad input
   - Check error rates remain consistent without crashes
