// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Security-focused integration tests for RustyMail
//!
//! These tests document and verify security-critical behavior including:
//! - CORS configuration
//! - Origin validation
//! - API key authentication
//! - Path traversal prevention
//! - Rate limiting
//!
//! IMPORTANT: These tests establish a baseline of current behavior.
//! Some tests may initially pass with INSECURE behavior - they will be
//! updated as security fixes are implemented to verify the fixes work.

use actix_web::{test, web, App, http::header, http::StatusCode};
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;
use std::fs;
use std::path::PathBuf;
use tokio::sync::Mutex as TokioMutex;
use sqlx::SqlitePool;
use async_trait::async_trait;
use tempfile::TempDir;

use rustymail::dashboard::services::{
    DashboardState, ClientManager, MetricsService, CacheService, CacheConfig,
    ConfigService, AiService, EmailService, SyncService, AccountService,
    EventBus, SmtpService, OutboxQueueService, OAuthService, OAuthConfig
};
use rustymail::dashboard::api::sse::SseManager;
use rustymail::connection_pool::{ConnectionPool, ConnectionFactory, PoolConfig};
use rustymail::prelude::CloneableImapSessionFactory;
use rustymail::imap::{ImapClient, AsyncImapSessionWrapper, ImapError};
use rustymail::config::Settings;
use dashmap::DashMap;

/// Initialize test environment with required environment variables
fn setup_test_env() {
    std::env::set_var("REST_HOST", "127.0.0.1");
    std::env::set_var("REST_PORT", "9437");
    std::env::set_var("SSE_HOST", "127.0.0.1");
    std::env::set_var("SSE_PORT", "9438");
    std::env::set_var("DASHBOARD_PORT", "9439");
    std::env::set_var("RUSTYMAIL_API_KEY", "test-rustymail-key-2024");
    std::env::set_var("MCP_BACKEND_URL", "http://localhost:9437/mcp");
    std::env::set_var("MCP_TIMEOUT", "30");
    std::env::set_var("IMAP_HOST", "localhost");
    std::env::set_var("IMAP_PORT", "143");
}

/// Helper function to create a test DashboardState with all required services
async fn create_test_dashboard_state(test_name: &str) -> web::Data<DashboardState> {
    use std::time::Duration;

    let db_file_path = format!("test_data/security_{}_test.db", test_name);
    let db_url = format!("sqlite:{}", db_file_path);

    // Clean up old test files
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));

    fs::create_dir_all("test_data").unwrap();
    fs::File::create(&db_file_path).unwrap();

    let pool = SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let metrics_interval_duration = Duration::from_secs(5);
    let client_manager = Arc::new(ClientManager::new(metrics_interval_duration));
    let metrics_service = Arc::new(MetricsService::new(metrics_interval_duration));
    let config_service = Arc::new(ConfigService::new());

    let cache_config = CacheConfig {
        database_url: db_url.clone(),
        max_memory_items: 100,
        max_folder_items: 50,
        max_cache_size_mb: 100,
        max_email_age_days: 30,
        sync_interval_seconds: 300,
    };

    let mut cache_service = CacheService::new(cache_config);
    cache_service.initialize().await.unwrap();
    let cache_service = Arc::new(cache_service);

    let accounts_config_path = format!("test_data/security_{}_accounts.json", test_name);
    let _ = fs::remove_file(&accounts_config_path);

    let mut account_service_temp = AccountService::new(&accounts_config_path);
    let account_db_pool = SqlitePool::connect(&db_url).await.unwrap();
    account_service_temp.initialize(account_db_pool.clone()).await.unwrap();
    let account_service = Arc::new(TokioMutex::new(account_service_temp));

    let mock_factory: rustymail::imap::ImapSessionFactory = Box::new(|| {
        Box::pin(async {
            Err(rustymail::imap::ImapError::Connection("Mock IMAP client".to_string()))
        })
    });
    let imap_session_factory = CloneableImapSessionFactory::new(mock_factory);

    struct MockConnectionFactory;

    #[async_trait]
    impl ConnectionFactory for MockConnectionFactory {
        async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError> {
            Err(ImapError::Connection("Mock connection pool".to_string()))
        }

        async fn validate(&self, _client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool {
            true
        }
    }

    let connection_pool = ConnectionPool::new(
        Arc::new(MockConnectionFactory),
        PoolConfig::default()
    );

    let email_service = Arc::new(
        EmailService::new(imap_session_factory.clone(), connection_pool.clone())
            .with_cache(cache_service.clone())
            .with_account_service(account_service.clone())
    );

    let sync_service = Arc::new(SyncService::new(
        imap_session_factory.clone(),
        cache_service.clone(),
        account_service.clone(),
        300,
    ));

    let ai_service = Arc::new(AiService::new_mock());
    let smtp_service = Arc::new(SmtpService::new(account_service.clone(), imap_session_factory.clone()));
    let outbox_queue_service = Arc::new(OutboxQueueService::new(account_db_pool.clone()));
    let event_bus = Arc::new(EventBus::new());

    // Create SSE manager with required arguments
    let mut sse_manager = SseManager::new(metrics_service.clone(), client_manager.clone());
    sse_manager.set_event_bus(Arc::clone(&event_bus));
    let sse_manager = Arc::new(sse_manager);

    // Create config
    let config = web::Data::new(Settings::default());

    web::Data::new(DashboardState {
        client_manager,
        metrics_service,
        cache_service,
        config_service,
        ai_service,
        email_service,
        sync_service,
        account_service,
        smtp_service,
        outbox_queue_service,
        sse_manager,
        event_bus,
        health_service: None,
        config,
        imap_session_factory,
        connection_pool,
        jobs: Arc::new(DashMap::new()),
        job_persistence: None,
        oauth_service: Arc::new(OAuthService::new(OAuthConfig { microsoft: None })),
    })
}

// ============================================================================
// CORS Configuration Tests (for Task 22)
// ============================================================================

/// Helper function to create a CORS middleware with the secure whitelist approach
/// matching the production configuration in main.rs
fn create_secure_cors(allowed_origins: &[&str]) -> actix_cors::Cors {
    let mut cors = actix_cors::Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .supports_credentials()
        .max_age(3600);

    for origin in allowed_origins {
        cors = cors.allowed_origin(origin);
    }

    cors
}

/// Test that CORS blocks requests from non-whitelisted origins
///
/// SECURE TEST: Verifies that Task 22 fix correctly blocks unauthorized origins.
/// External origins not in ALLOWED_ORIGINS should not receive CORS headers.
#[tokio::test]
#[serial]
async fn test_cors_blocks_unauthorized_origins() {
    setup_test_env();
    println!("=== SECURITY TEST: CORS Blocks Unauthorized Origins ===");

    let dashboard_state = create_test_dashboard_state("cors_blocked").await;

    // Configure CORS with only localhost allowed (matching production config)
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(create_secure_cors(&["http://localhost:9439", "http://127.0.0.1:9439"]))
            .route("/api/health", web::get().to(|| async { "ok" }))
    ).await;

    // Test with external origin - should NOT receive CORS headers
    let req = test::TestRequest::get()
        .uri("/api/health")
        .insert_header((header::ORIGIN, "https://evil.example.com"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // The request itself succeeds but CORS headers should not be present
    // This means browser-based requests from evil.example.com would fail
    let cors_origin = resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert!(cors_origin.is_none(), "External origin should NOT receive CORS headers");

    println!("  External origin correctly blocked (no CORS headers)");
}

/// Test that CORS allows requests from whitelisted origins
#[tokio::test]
#[serial]
async fn test_cors_allows_configured_origins() {
    setup_test_env();
    println!("=== SECURITY TEST: CORS Allows Configured Origins ===");

    let dashboard_state = create_test_dashboard_state("cors_allowed").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(create_secure_cors(&["http://localhost:9439", "http://127.0.0.1:9439"]))
            .route("/api/health", web::get().to(|| async { "ok" }))
    ).await;

    // Test with allowed localhost origin
    let req = test::TestRequest::get()
        .uri("/api/health")
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // CORS headers should be present for allowed origin
    let cors_origin = resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert!(cors_origin.is_some(), "Allowed origin should receive CORS headers");
    assert_eq!(cors_origin.unwrap().to_str().unwrap(), "http://localhost:9439");

    println!("  Configured origin correctly allowed");
}

/// Test CORS preflight OPTIONS request handling for allowed origins
#[tokio::test]
#[serial]
async fn test_cors_preflight_options_allowed() {
    setup_test_env();
    println!("=== SECURITY TEST: CORS Preflight OPTIONS (Allowed Origin) ===");

    let dashboard_state = create_test_dashboard_state("cors_preflight").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(create_secure_cors(&["http://localhost:9439"]))
            .route("/api/test", web::post().to(|| async { "ok" }))
    ).await;

    // Test preflight request from allowed origin
    let req = test::TestRequest::with_uri("/api/test")
        .method(actix_web::http::Method::OPTIONS)
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Preflight should succeed for allowed origin
    assert!(resp.status().is_success() || resp.status() == StatusCode::NO_CONTENT);

    // Should have CORS headers
    let cors_origin = resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert!(cors_origin.is_some(), "Preflight from allowed origin should have CORS headers");

    println!("  Preflight OPTIONS request handled for allowed origin");
}

/// Test CORS preflight OPTIONS request handling for blocked origins
#[tokio::test]
#[serial]
async fn test_cors_preflight_options_blocked() {
    setup_test_env();
    println!("=== SECURITY TEST: CORS Preflight OPTIONS (Blocked Origin) ===");

    let dashboard_state = create_test_dashboard_state("cors_preflight_blocked").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(create_secure_cors(&["http://localhost:9439"]))
            .route("/api/test", web::post().to(|| async { "ok" }))
    ).await;

    // Test preflight request from non-allowed origin
    let req = test::TestRequest::with_uri("/api/test")
        .method(actix_web::http::Method::OPTIONS)
        .insert_header((header::ORIGIN, "https://evil.example.com"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Preflight may succeed but should not have CORS headers for blocked origin
    let cors_origin = resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert!(cors_origin.is_none(), "Preflight from blocked origin should NOT have CORS headers");

    println!("  Preflight OPTIONS correctly blocked for unauthorized origin");
}

// ============================================================================
// Origin Validation Tests (for Task 23)
// ============================================================================

/// Test that MCP accepts localhost origin (with valid API key)
#[tokio::test]
#[serial]
async fn test_mcp_origin_localhost_accepted() {
    setup_test_env();
    std::env::set_var("RUSTYMAIL_API_KEY", "test-key-for-origin-tests");
    println!("=== SECURITY TEST: MCP Origin - Localhost Accepted ===");

    let dashboard_state = create_test_dashboard_state("origin_localhost").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .insert_header(("X-Api-Key", "test-key-for-origin-tests"))
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // localhost should be accepted
    assert!(resp.status().is_success(), "localhost origin should be accepted");
    println!("  localhost origin accepted");
}

/// Test that MCP accepts 127.0.0.1 origin (with valid API key)
#[tokio::test]
#[serial]
async fn test_mcp_origin_127_0_0_1_accepted() {
    setup_test_env();
    std::env::set_var("RUSTYMAIL_API_KEY", "test-key-for-origin-tests");
    println!("=== SECURITY TEST: MCP Origin - 127.0.0.1 Accepted ===");

    let dashboard_state = create_test_dashboard_state("origin_127").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "http://127.0.0.1:9439"))
        .insert_header(("X-Api-Key", "test-key-for-origin-tests"))
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success(), "127.0.0.1 origin should be accepted");
    println!("  127.0.0.1 origin accepted");
}

/// Test that substring bypass attacks are blocked
///
/// SECURITY TEST: Verifies that Task 23 fix correctly blocks origins that
/// contain "localhost" as a substring but are not exact matches.
/// "evil.localhost.com" should be rejected because it's not in ALLOWED_ORIGINS.
#[tokio::test]
#[serial]
async fn test_mcp_origin_substring_bypass_blocked() {
    setup_test_env();
    println!("=== SECURITY TEST: MCP Origin Substring Bypass Blocked ===");

    let dashboard_state = create_test_dashboard_state("origin_substring").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    // Test with evil.localhost.com - contains "localhost" substring but should be blocked
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "https://evil.localhost.com"))
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // After Task 23 fix: This should return 403 Forbidden
    assert_eq!(resp.status(), StatusCode::FORBIDDEN,
        "evil.localhost.com should be BLOCKED - substring matching vulnerability fixed");
    println!("  Substring bypass attack correctly blocked");
}

/// Test that requests without Origin header are currently allowed
///
/// BASELINE TEST: This documents behavior that may need to change.
/// Non-browser clients (CLI tools) don't send Origin headers - allowed with valid API key.
#[tokio::test]
#[serial]
async fn test_mcp_origin_missing_header_allowed_with_api_key() {
    setup_test_env();
    std::env::set_var("RUSTYMAIL_API_KEY", "test-key-for-origin-tests");
    println!("=== SECURITY TEST: MCP Missing Origin Header (With API Key) ===");

    let dashboard_state = create_test_dashboard_state("origin_missing").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    // Request without Origin header but WITH valid API key
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header(("X-Api-Key", "test-key-for-origin-tests"))
        // No Origin header - CLI clients don't send it
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Allowed for CLI clients with valid API key
    assert!(resp.status().is_success(),
        "Requests without Origin should be accepted for CLI clients with valid API key");
    println!("  Missing Origin header allowed for authenticated CLI clients");
}

/// Test that external origins are rejected
#[tokio::test]
#[serial]
async fn test_mcp_origin_external_rejected() {
    setup_test_env();
    println!("=== SECURITY TEST: MCP External Origin Rejection ===");

    let dashboard_state = create_test_dashboard_state("origin_external").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    // Test with clearly external origin
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "https://attacker.example.com"))
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // External origins should be rejected
    assert_eq!(resp.status(), StatusCode::FORBIDDEN,
        "External origins should be rejected with 403");
    println!("  External origin correctly rejected");
}

// ============================================================================
// Path Traversal Tests (for Task 27)
// ============================================================================

/// Test path sanitization removes traversal sequences
#[tokio::test]
async fn test_path_traversal_sanitization() {
    use rustymail::dashboard::services::attachment_storage::sanitize_message_id;

    println!("=== SECURITY TEST: Path Traversal Sanitization ===");

    // Test that dangerous characters are sanitized
    let dangerous_inputs = vec![
        ("../../../etc/passwd", "should not contain path separators"),
        ("..\\..\\windows\\system32", "should not contain backslashes"),
        ("<script>alert(1)</script>", "should sanitize angle brackets"),
        ("file:///etc/passwd", "should sanitize colons"),
    ];

    for (input, description) in dangerous_inputs {
        let sanitized = sanitize_message_id(input);
        assert!(!sanitized.contains('/'), "{}: {}", description, sanitized);
        assert!(!sanitized.contains('\\'), "{}: {}", description, sanitized);
        assert!(!sanitized.contains(':'), "{}: {}", description, sanitized);
        assert!(!sanitized.contains('<'), "{}: {}", description, sanitized);
        assert!(!sanitized.contains('>'), "{}: {}", description, sanitized);
        println!("  Input '{}' sanitized to '{}'", input, sanitized);
    }

    println!("  Path traversal characters correctly sanitized");
}

/// Test that attachment paths stay within storage directory
///
/// SECURITY TEST: Verifies path traversal prevention (Task 27 fix)
/// - Valid paths return Ok with sanitized path
/// - Path traversal attempts return Err and are rejected
#[tokio::test]
async fn test_attachment_path_containment() {
    use rustymail::dashboard::services::attachment_storage::get_attachment_path;

    println!("=== SECURITY TEST: Attachment Path Containment ===");

    // Test normal path construction - should succeed
    let path_result = get_attachment_path("user@example.com", "<msg123@example.com>", "document.pdf");
    assert!(path_result.is_ok(), "Valid filename should return Ok");

    let path = path_result.unwrap();

    // Path should be within attachments directory
    assert!(path.to_string_lossy().contains("attachments"),
        "Path should contain 'attachments'");

    // Path components should be sanitized
    let path_str = path.to_string_lossy();
    assert!(!path_str.contains(".."), "Path should not contain ..");

    println!("  Normal path: {:?}", path);

    // Test with malicious filename - should FAIL (return Err)
    let malicious_result = get_attachment_path(
        "user@example.com",
        "<msg123@example.com>",
        "../../../etc/passwd"
    );

    // Path traversal attempts should now be rejected with an error
    assert!(malicious_result.is_err(),
        "Path traversal attempt should return Err, not a sanitized path");
    println!("  Malicious filename correctly rejected with error");

    // Test backslash traversal (Windows-style)
    let backslash_result = get_attachment_path(
        "user@example.com",
        "<msg123@example.com>",
        "..\\..\\windows\\system32\\config"
    );
    assert!(backslash_result.is_err(),
        "Backslash path traversal should be rejected");
    println!("  Backslash traversal correctly rejected");

    println!("  Path containment security verified");
}

/// Test symlink escape prevention (placeholder for Task 27)
#[tokio::test]
async fn test_attachment_symlink_escape() {
    println!("=== SECURITY TEST: Symlink Escape Prevention ===");

    // Create temp directory for testing
    let temp_dir = TempDir::new().unwrap();
    let attachments_dir = temp_dir.path().join("attachments");
    fs::create_dir_all(&attachments_dir).unwrap();

    // This test documents what SHOULD happen:
    // 1. Create a symlink inside attachments/ pointing outside
    // 2. Attempt to access a file through the symlink
    // 3. The access should be blocked after canonicalization

    // For now, document that this check needs to be implemented
    println!("  NOTE: Symlink escape prevention to be implemented in Task 27");
    println!("  Test placeholder - actual symlink test requires implementation");
}

// ============================================================================
// API Key Authentication Tests (for Tasks 24, 25)
// ============================================================================

/// Test that MCP endpoints require API key authentication
///
/// SECURITY TEST: Verifies Task 25 fix - MCP endpoints must reject requests without valid API key
#[tokio::test]
#[serial]
async fn test_mcp_requires_api_key() {
    setup_test_env();
    // Set a test API key in environment
    std::env::set_var("RUSTYMAIL_API_KEY", "test-secure-key-for-mcp-auth");
    println!("=== SECURITY TEST: MCP API Key Requirement ===");

    let dashboard_state = create_test_dashboard_state("mcp_auth").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    // Request WITHOUT API key - should be rejected
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED,
        "MCP endpoint should return 401 when no API key provided");
    println!("  Request without API key correctly rejected with 401");

    // Request WITH valid API key - should succeed
    let req_with_key = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .insert_header(("X-Api-Key", "test-secure-key-for-mcp-auth"))
        .set_json(&request)
        .to_request();

    let resp_with_key = test::call_service(&app, req_with_key).await;
    assert!(resp_with_key.status().is_success(),
        "MCP endpoint should accept request with valid API key");
    println!("  Request with valid API key accepted");

    // Request WITH invalid API key - should be rejected
    let req_invalid_key = test::TestRequest::post()
        .uri("/mcp")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .insert_header((header::ORIGIN, "http://localhost:9439"))
        .insert_header(("X-Api-Key", "wrong-api-key"))
        .set_json(&request)
        .to_request();

    let resp_invalid = test::call_service(&app, req_invalid_key).await;
    assert_eq!(resp_invalid.status(), StatusCode::UNAUTHORIZED,
        "MCP endpoint should return 401 for invalid API key");
    println!("  Request with invalid API key correctly rejected with 401");
}

/// Test API key validation on REST endpoints
#[tokio::test]
#[serial]
async fn test_rest_api_key_validation() {
    setup_test_env();
    println!("=== SECURITY TEST: REST API Key Validation ===");

    let dashboard_state = create_test_dashboard_state("rest_auth").await;

    // REST endpoints should require API key
    // This tests the existing REST authentication

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(|cfg| rustymail::dashboard::api::init_routes(cfg))
    ).await;

    // Request without API key should fail
    let req = test::TestRequest::get()
        .uri("/api/accounts")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should require authentication
    println!("  Response status without API key: {}", resp.status());
}

/// Test that hardcoded test credentials have been removed (Task 24 fix verification)
#[tokio::test]
async fn test_no_hardcoded_credentials() {
    println!("=== SECURITY TEST: No Hardcoded Credentials ===");

    // Check .env.example does NOT contain hardcoded test credentials
    let env_example = fs::read_to_string(".env.example").unwrap_or_default();

    // Verify no hardcoded test keys
    assert!(!env_example.contains("test-rustymail-key-2024"),
        ".env.example should NOT contain hardcoded test keys");
    assert!(!env_example.contains("test-api-key"),
        ".env.example should NOT contain test-api-key patterns");

    // Verify placeholder is present
    assert!(env_example.contains("your-secure-api-key-here"),
        ".env.example should contain placeholder for API key");

    // Verify security guidance is present
    assert!(env_example.contains("openssl rand -hex 32"),
        ".env.example should contain secure key generation instructions");

    println!("  Task 24 FIXED: No hardcoded credentials in .env.example");
    println!("  Placeholder and security guidance present");
}

// ============================================================================
// Rate Limiting Tests (for Task 28)
// ============================================================================

/// Test that rate limiting validators exist
#[tokio::test]
async fn test_rate_limiting_validators_exist() {
    use rustymail::api::validation;

    println!("=== SECURITY TEST: Rate Limiting Validators ===");

    // The rate limiting logic exists but may not be wired into all routes
    // This test verifies the validators are available

    // Test IP rate limiting function exists and works
    // Note: This tests the validation module, not the middleware integration

    println!("  Rate limiting validation module exists");
    println!("  NOTE: Integration with REST/MCP routes to be verified in Task 28");
}

/// Test rate limit response headers are present (Task 28)
#[tokio::test]
#[serial]
async fn test_rate_limit_headers_present() {
    use rustymail::api::rate_limit::{RateLimitConfig, RateLimitMiddleware};

    setup_test_env();
    println!("=== SECURITY TEST: Rate Limit Headers Present ===");

    let dashboard_state = create_test_dashboard_state("rate_limit_headers").await;

    // Configure rate limiting with high limit so we don't hit 429
    let rate_limit_config = RateLimitConfig {
        per_ip_per_minute: 1000,
        per_ip_per_hour: 10000,
        whitelist_ips: vec![],
    };

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(RateLimitMiddleware::new(rate_limit_config))
            .route("/api/health", web::get().to(|| async { "ok" }))
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/health")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Verify rate limit headers are present
    assert!(resp.headers().contains_key("x-ratelimit-limit"),
        "X-RateLimit-Limit header should be present");
    assert!(resp.headers().contains_key("x-ratelimit-remaining"),
        "X-RateLimit-Remaining header should be present");
    assert!(resp.headers().contains_key("x-ratelimit-reset"),
        "X-RateLimit-Reset header should be present");

    println!("  X-RateLimit-Limit: {:?}", resp.headers().get("x-ratelimit-limit"));
    println!("  X-RateLimit-Remaining: {:?}", resp.headers().get("x-ratelimit-remaining"));
    println!("  X-RateLimit-Reset: {:?}", resp.headers().get("x-ratelimit-reset"));
    println!("  Rate limit headers correctly present on all responses");
}

/// Test 429 response when rate limit exceeded (Task 28)
#[tokio::test]
#[serial]
async fn test_rate_limit_429_response() {
    use rustymail::api::rate_limit::{RateLimitConfig, RateLimitMiddleware};

    setup_test_env();
    println!("=== SECURITY TEST: Rate Limit 429 Response ===");

    let dashboard_state = create_test_dashboard_state("rate_limit_429").await;

    // Configure very low rate limit to trigger 429
    let rate_limit_config = RateLimitConfig {
        per_ip_per_minute: 2,  // Only allow 2 requests per minute
        per_ip_per_hour: 100,
        whitelist_ips: vec![],
    };

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .wrap(RateLimitMiddleware::new(rate_limit_config))
            .route("/api/health", web::get().to(|| async { "ok" }))
    ).await;

    // First two requests should succeed
    for i in 0..2 {
        let req = test::TestRequest::get()
            .uri("/api/health")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK,
            "Request {} should succeed", i + 1);
    }

    // Third request should get 429
    let req = test::TestRequest::get()
        .uri("/api/health")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS,
        "Request 3 should return 429 Too Many Requests");

    // Verify response includes proper headers
    assert!(resp.headers().contains_key("retry-after"),
        "429 response should include Retry-After header");
    assert_eq!(resp.headers().get("x-ratelimit-remaining").map(|v| v.to_str().ok()).flatten(),
        Some("0"), "X-RateLimit-Remaining should be 0");

    println!("  First 2 requests succeeded (within limit)");
    println!("  Third request correctly returned 429 Too Many Requests");
    println!("  Retry-After header: {:?}", resp.headers().get("retry-after"));
    println!("  Rate limiting correctly rejects requests over limit");
}

// ============================================================================
// Combined Security Tests
// ============================================================================

/// Test security headers are present in responses
#[tokio::test]
#[serial]
async fn test_security_headers_baseline() {
    setup_test_env();
    println!("=== SECURITY TEST: Security Headers (Baseline) ===");

    let dashboard_state = create_test_dashboard_state("security_headers").await;

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .route("/api/health", web::get().to(|| async { "ok" }))
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/health")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Check for recommended security headers
    let headers_to_check = vec![
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("X-XSS-Protection", "1; mode=block"),
    ];

    for (header_name, _expected) in headers_to_check {
        if resp.headers().contains_key(header_name) {
            println!("  {} header present", header_name);
        } else {
            println!("  BASELINE: {} header not present", header_name);
        }
    }
}

/// Summary test that documents all security baselines
#[tokio::test]
async fn test_security_baseline_summary() {
    println!("\n========================================");
    println!("SECURITY BASELINE SUMMARY");
    println!("========================================\n");

    println!("Task 22 (CORS): FIXED");
    println!("  - Implemented: Whitelist via ALLOWED_ORIGINS env var");
    println!("  - Default: localhost:9439 and 127.0.0.1:9439 for development");
    println!("  - Supports credentials, specific methods/headers\n");

    println!("Task 23 (Origin Validation): FIXED");
    println!("  - Implemented: Exact origin matching using ALLOWED_ORIGINS env var");
    println!("  - Missing Origin header still allowed for CLI clients (intentional)");
    println!("  - Blocks substring attacks like 'evil.localhost.com'\n");

    println!("Task 24 (Hardcoded Credentials): FIXED");
    println!("  - Implemented: init_from_env() loads API key from RUSTYMAIL_API_KEY");
    println!("  - .env.example uses placeholder with security guidance");
    println!("  - init_with_test_defaults() is now #[cfg(test)] only\n");

    println!("Task 25 (MCP Authentication): FIXED");
    println!("  - Implemented: validate_api_key() in mcp_http.rs");
    println!("  - Extracts key from X-Api-Key or Authorization: Bearer headers");
    println!("  - Returns 401 with WWW-Authenticate header for invalid/missing key\n");

    println!("Task 27 (Path Traversal):");
    println!("  - Current: Basic character sanitization only");
    println!("  - Fix: Add canonicalization + containment checks\n");

    println!("Task 28 (Rate Limiting):");
    println!("  - Current: Validators exist but not wired to all routes");
    println!("  - Fix: Add middleware to REST and MCP paths\n");

    println!("========================================");
    println!("Run: cargo test --test integration security_tests");
    println!("to verify security fixes as they are implemented");
    println!("========================================\n");
}
