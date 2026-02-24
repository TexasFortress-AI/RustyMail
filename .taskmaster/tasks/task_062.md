# Task ID: 62

**Title:** Upgrade sqlx from 0.7 to 0.8+

**Status:** deferred

**Dependencies:** 1 ✓

**Priority:** medium

**Description:** Upgrade sqlx dependency from version 0.7 to 0.8+ to address critical security vulnerabilities RUSTSEC-2024-0363 (binary protocol cast overflow) and RUSTSEC-2023-0071 (RSA timing attack via sqlx-mysql), while also resolving unmaintained transitive dependencies paste and proc-macro-error.

**Details:**

1. Update Cargo.toml dependencies:
```toml
[dependencies]
sqlx = { version = "0.8.2", features = ["runtime-tokio-rustls", "postgres", "mysql", "sqlite", "macros", "migrate", "chrono", "uuid"] }
```

2. Migration changes required:
   a) Query macro syntax updates:
   ```rust
   // Old (0.7)
   let row = sqlx::query!("SELECT * FROM users WHERE id = ?", id)
       .fetch_one(&pool).await?;
   
   // New (0.8)
   let row = sqlx::query!("SELECT * FROM users WHERE id = $1", id)
       .fetch_one(&pool).await?;
   ```
   
   b) Connection pool API changes:
   ```rust
   // Old (0.7)
   let pool = PgPoolOptions::new()
       .max_connections(5)
       .connect(&database_url).await?;
   
   // New (0.8)
   let pool = PgPoolOptions::new()
       .max_connections(5)
       .acquire_timeout(Duration::from_secs(3))
       .connect(&database_url).await?;
   ```
   
   c) Migration runner updates:
   ```rust
   // Old (0.7)
   sqlx::migrate!("./migrations")
       .run(&pool).await?;
   
   // New (0.8)
   sqlx::migrate!("./migrations")
       .set_ignore_missing(true)
       .run(&pool).await?;
   ```

3. Update all database query macros throughout the codebase:
   - Search for `query!`, `query_as!`, `query_scalar!` macros
   - Update parameter placeholders from `?` to `$1`, `$2`, etc. for PostgreSQL
   - Keep `?` for MySQL/SQLite if used

4. Update error handling:
   ```rust
   // New error types in 0.8
   use sqlx::error::ErrorKind;
   match err.kind() {
       ErrorKind::UniqueViolation => // handle duplicate key
       ErrorKind::ForeignKeyViolation => // handle FK constraint
       _ => // other errors
   }
   ```

5. Review and update any custom type implementations:
   - Check `FromRow`, `Type`, `Encode`, `Decode` trait implementations
   - Update for any API changes in 0.8

6. Update migration files if needed:
   - Review migrations/001-004 for any sqlx-specific syntax
   - Ensure compatibility with new migration runner

7. Address breaking changes in connection handling:
   - Review connection lifecycle management
   - Update any custom connection wrappers
   - Check transaction handling code

8. Clean build and resolve compilation errors:
   ```bash
   cargo clean
   cargo build
   cargo sqlx prepare --check
   ```

**Test Strategy:**

1. Run full test suite to catch any query macro compilation errors:
   ```bash
   cargo test --all-features
   ```

2. Verify database migrations still apply correctly:
   ```bash
   cargo sqlx migrate run
   cargo sqlx migrate info
   ```

3. Test connection pool behavior under load:
   - Create stress test spawning 100+ concurrent queries
   - Verify connection timeout handling
   - Check pool exhaustion behavior

4. Validate security fixes are applied:
   ```bash
   cargo audit
   # Verify RUSTSEC-2024-0363 and RUSTSEC-2023-0071 are resolved
   ```

5. Test all database operations:
   - Account CRUD operations
   - Email synchronization queries
   - Attachment metadata queries
   - AI model configuration queries

6. Performance regression testing:
   - Compare query execution times before/after upgrade
   - Monitor connection pool metrics
   - Check for any memory leaks with valgrind

7. Integration testing with live database:
   - Test OAuth account creation/updates
   - Verify email sync still works correctly
   - Test concurrent operations

8. Verify transitive dependencies are updated:
   ```bash
   cargo tree | grep -E "paste|proc-macro-error"
   # Should show no results or updated versions
   ```
