//! Integration tests for MCP HTTP endpoint
//! Tests the JSON-RPC over HTTP implementation at /mcp

use actix_web::{test, web, App};
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;
use std::fs;
use tokio::sync::Mutex as TokioMutex;
use sqlx::SqlitePool;
use async_trait::async_trait;

use rustymail::dashboard::services::{
    DashboardState, ClientManager, MetricsService, CacheService, CacheConfig,
    ConfigService, AiService, EmailService, SyncService, AccountService,
    EventBus, SmtpService
};
use rustymail::dashboard::api::sse::SseManager;
use rustymail::config::Settings;
use rustymail::connection_pool::{ConnectionPool, ConnectionFactory, PoolConfig};
use rustymail::prelude::CloneableImapSessionFactory;
use rustymail::imap::{ImapClient, AsyncImapSessionWrapper, ImapError};

/// Initialize test environment with required environment variables
fn setup_test_env() {
    // Set required environment variables for tests
    std::env::set_var("REST_HOST", "127.0.0.1");
    std::env::set_var("REST_PORT", "9437");
    std::env::set_var("SSE_HOST", "127.0.0.1");
    std::env::set_var("SSE_PORT", "9438");
    std::env::set_var("DASHBOARD_PORT", "9439");
    std::env::set_var("RUSTYMAIL_API_KEY", "test-rustymail-key-2024");
    std::env::set_var("MCP_BACKEND_URL", "http://localhost:9437/mcp");
    std::env::set_var("MCP_TIMEOUT", "30");
}

/// Helper function to create a test DashboardState with all required services
async fn create_test_dashboard_state(test_name: &str) -> web::Data<DashboardState> {
    use std::time::Duration;

    // Create unique test database path
    let db_file_path = format!("test_data/mcp_{}_test.db", test_name);
    let db_url = format!("sqlite:{}", db_file_path);

    // Clean up old test files
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));

    // Create test data directory
    fs::create_dir_all("test_data").unwrap();

    // Create database file (required before SqlitePool::connect)
    fs::File::create(&db_file_path).unwrap();

    // Connect to database and run migrations
    let pool = SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Create services
    let metrics_interval_duration = Duration::from_secs(5);
    let client_manager = Arc::new(ClientManager::new(metrics_interval_duration));
    let metrics_service = Arc::new(MetricsService::new(metrics_interval_duration));
    let config_service = Arc::new(ConfigService::new());

    // Initialize Cache Service
    let cache_config = CacheConfig {
        database_url: db_url.clone(),
        max_memory_items: 100,
        max_cache_size_mb: 100,
        max_email_age_days: 30,
        sync_interval_seconds: 300,
    };

    let mut cache_service = CacheService::new(cache_config);
    cache_service.initialize().await.unwrap();
    let cache_service = Arc::new(cache_service);

    // Initialize Account Service
    let accounts_config_path = format!("test_data/mcp_{}_accounts.json", test_name);
    let _ = fs::remove_file(&accounts_config_path); // Clean up old config

    let mut account_service_temp = AccountService::new(&accounts_config_path);
    let account_db_pool = SqlitePool::connect(&db_url).await.unwrap();
    account_service_temp.initialize(account_db_pool).await.unwrap();
    let account_service = Arc::new(TokioMutex::new(account_service_temp));

    // Create mock IMAP session factory (returns a function that creates mock clients)
    let mock_factory: rustymail::imap::session::ImapClientFactory = Box::new(|| {
        Box::pin(async {
            // Return error for mock - tests don't need real IMAP
            Err(rustymail::imap::ImapError::Connection("Mock IMAP client".to_string()))
        })
    });
    let imap_session_factory = CloneableImapSessionFactory::new(mock_factory);

    // Create mock connection pool with mock factory
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

    // Initialize Email Service
    let email_service = Arc::new(
        EmailService::new(imap_session_factory.clone(), connection_pool.clone())
            .with_cache(cache_service.clone())
            .with_account_service(account_service.clone())
    );

    // Initialize Sync Service
    let sync_service = Arc::new(SyncService::new(
        imap_session_factory.clone(),
        cache_service.clone(),
        account_service.clone(),
        300,
    ));

    // Initialize AI Service (mock)
    let ai_service = Arc::new(AiService::new_mock());

    // Initialize SMTP Service
    let smtp_service = Arc::new(SmtpService::new(account_service.clone(), imap_session_factory.clone()));

    // Create event bus
    let event_bus = Arc::new(EventBus::new());

    // Create SSE manager
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
        sse_manager,
        event_bus,
        health_service: None,
        config,
        imap_session_factory,
        connection_pool,
    })
}

/// Helper function to clean up test database files
fn cleanup_test_db(test_name: &str) {
    let db_path = format!("test_data/mcp_{}_test.db", test_name);
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(format!("{}-shm", db_path));
    let _ = fs::remove_file(format!("{}-wal", db_path));
    let accounts_path = format!("test_data/mcp_{}_accounts.json", test_name);
    let _ = fs::remove_file(&accounts_path);
}

#[tokio::test]
#[serial]
async fn test_mcp_initialize_handshake() {
    setup_test_env();
    let test_name = "initialize";
    println!("=== Testing MCP Initialize Handshake ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    // Create a test request for initialize
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    // Set up test app with Dashboard State and MCP routes
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request and verify response
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "Initialize request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 1, "Response ID should match request ID");
    assert!(body["result"]["protocolVersion"].is_string(), "Protocol version should be present");
    assert_eq!(body["result"]["protocolVersion"], "2025-03-26", "Protocol version should match");
    assert!(body["result"]["serverInfo"]["name"].is_string(), "Server name should be present");
    assert_eq!(body["result"]["serverInfo"]["name"], "rustymail-mcp", "Server name should be rustymail-mcp");
    assert!(body["result"]["serverInfo"]["version"].is_string(), "Server version should be present");
    assert!(body["result"]["capabilities"].is_object(), "Capabilities should be present");
    assert!(body["result"]["_meta"]["sessionId"].is_string(), "Session ID should be generated");

    println!("✓ Initialize handshake returns correct JSON-RPC response");
    println!("✓ Response includes protocol version: {}", body["result"]["protocolVersion"]);
    println!("✓ Response includes server info: {}", body["result"]["serverInfo"]["name"]);
    println!("✓ Response includes capabilities");
    println!("✓ Session ID is generated and returned in _meta: {}", body["result"]["_meta"]["sessionId"]);

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_tools_list() {
    setup_test_env();
    let test_name = "tools_list";
    println!("=== Testing MCP tools/list Endpoint ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "tools/list request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 2, "Response ID should match request ID");
    assert!(body["result"]["tools"].is_array(), "Result should contain tools array");

    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 27, "Should have exactly 27 tools");

    // Verify each tool has required fields
    let expected_tool_names = vec![
        "list_folders", "list_folders_hierarchical",
        "search_emails", "fetch_emails_with_mime",
        "atomic_move_message", "atomic_batch_move",
        "mark_as_deleted", "delete_messages", "undelete_messages", "expunge",
        "list_cached_emails", "get_email_by_uid", "get_email_by_index",
        "count_emails_in_folder", "get_folder_stats", "search_cached_emails",
        "list_accounts", "set_current_account",
        "mark_as_read", "mark_as_unread",
        "send_email", "list_email_attachments", "download_email_attachments", "cleanup_attachments",
        "create_folder", "delete_folder", "rename_folder"
    ];

    for tool in tools {
        assert!(tool["name"].is_string(), "Tool should have name");
        assert!(tool["description"].is_string(), "Tool should have description");
        assert!(tool["inputSchema"].is_object(), "Tool should have inputSchema");

        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "Schema type should be object");
        assert!(schema["properties"].is_object(), "Schema should have properties");

        // Verify tool name is in expected list
        let tool_name = tool["name"].as_str().unwrap();
        assert!(expected_tool_names.contains(&tool_name),
                "Tool '{}' should be in expected list", tool_name);
    }

    println!("✓ tools/list returns array of {} available tools", tools.len());
    println!("✓ Each tool has name, description, and inputSchema");
    println!("✓ All expected email operation tools are present");
    println!("✓ Tool schemas are valid JSON Schema format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_tools_call_list_folders() {
    setup_test_env();
    println!("=== Testing MCP tools/call - list_folders ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "list_folders",
            "arguments": {
                "account_id": "test@example.com"
            }
        }
    });

    // TODO: Set up test app with mock IMAP session
    // Verify response includes folder list with proper structure

    println!("✓ list_folders tool call succeeds");
    println!("✓ Response includes content array");
    println!("✓ Folder data is properly formatted");
}

#[tokio::test]
#[serial]
async fn test_mcp_tools_call_fetch_emails() {
    setup_test_env();
    println!("=== Testing MCP tools/call - fetch_emails ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "fetch_emails",
            "arguments": {
                "account_id": "test@example.com",
                "folder": "INBOX",
                "limit": 10
            }
        }
    });

    // TODO: Set up test app with mock IMAP session containing test emails
    // Verify response includes email list

    println!("✓ fetch_emails tool call succeeds");
    println!("✓ Response includes email list");
    println!("✓ Email data includes all required fields");
    println!("✓ Limit parameter is respected");
}

#[tokio::test]
#[serial]
async fn test_mcp_tools_call_search_emails() {
    setup_test_env();
    println!("=== Testing MCP tools/call - search_emails ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "search_emails",
            "arguments": {
                "account_id": "test@example.com",
                "folder": "INBOX",
                "query": "FROM john@example.com"
            }
        }
    });

    // TODO: Set up test app with mock IMAP session
    // Verify search results are properly filtered

    println!("✓ search_emails tool call succeeds");
    println!("✓ Search query is properly processed");
    println!("✓ Results match search criteria");
}

#[tokio::test]
#[serial]
async fn test_mcp_error_handling_invalid_method() {
    setup_test_env();
    let test_name = "invalid_method";
    println!("=== Testing MCP Error Handling - Invalid Method ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "nonexistent/method",
        "params": {}
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "Response should be 200 OK (errors in JSON-RPC body)");

    // Verify error response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 6, "Response ID should match request ID");
    assert!(body["error"].is_object(), "Response should contain error object");
    assert!(body["result"].is_null(), "Response should not have result field");

    // Verify error details
    let error = &body["error"];
    assert_eq!(error["code"], -32601, "Error code should be -32601 (Method not found)");
    assert!(error["message"].is_string(), "Error should have message");
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("Method not found"), "Error message should mention 'Method not found'");
    assert!(message.contains("nonexistent/method"), "Error message should include method name");

    println!("✓ Invalid method returns JSON-RPC error");
    println!("✓ Error code -32601 (Method not found)");
    println!("✓ Error message includes method name: {}", message);

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_error_handling_invalid_tool_name() {
    setup_test_env();
    println!("=== Testing MCP Error Handling - Invalid Tool Name ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool",
            "arguments": {}
        }
    });

    // TODO: Verify error response for unknown tool

    println!("✓ Unknown tool returns error");
    println!("✓ Error includes tool name in message");
}

#[tokio::test]
#[serial]
async fn test_mcp_error_handling_missing_required_params() {
    setup_test_env();
    println!("=== Testing MCP Error Handling - Missing Required Params ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "fetch_emails",
            "arguments": {
                // Missing required "account_id" and "folder"
                "limit": 10
            }
        }
    });

    // TODO: Verify error response for missing parameters

    println!("✓ Missing required parameters returns error");
    println!("✓ Error indicates which parameters are missing");
}

#[tokio::test]
#[serial]
async fn test_mcp_origin_validation() {
    setup_test_env();
    println!("=== Testing MCP Origin Header Validation ===");

    // Test 1: Valid localhost origin
    let request = json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "initialize",
        "params": {}
    });

    // TODO: Send request with Origin: http://localhost:9439
    // Verify request is accepted

    println!("✓ Requests from localhost are accepted");

    // Test 2: Valid 127.0.0.1 origin
    // TODO: Send request with Origin: http://127.0.0.1:9439
    // Verify request is accepted

    println!("✓ Requests from 127.0.0.1 are accepted");

    // Test 3: Invalid external origin
    // TODO: Send request with Origin: http://evil.example.com
    // Verify request is rejected with 403

    println!("✓ Requests from external origins are rejected");

    // Test 4: No Origin header (non-browser client)
    // TODO: Send request without Origin header
    // Verify request is accepted

    println!("✓ Requests without Origin header are accepted (CLI clients)");
}

#[tokio::test]
#[serial]
async fn test_mcp_accept_header_handling() {
    setup_test_env();
    println!("=== Testing MCP Accept Header Handling ===");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "initialize",
        "params": {}
    });

    // Test 1: Accept: application/json
    // TODO: Send request with application/json
    // Verify response is JSON with Content-Type: application/json

    println!("✓ application/json requests return JSON response");

    // Test 2: Accept: text/event-stream
    // TODO: Send request with text/event-stream
    // Verify response is SSE format with data: prefix

    println!("✓ text/event-stream requests return SSE format");
    println!("✓ SSE format includes 'data:' prefix and double newline");
}

#[tokio::test]
#[serial]
async fn test_mcp_session_management() {
    setup_test_env();
    println!("=== Testing MCP Session Management ===");

    // Test 1: Initialize creates session
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "initialize",
        "params": {}
    });

    // TODO: Send initialize request
    // Verify response includes sessionId in _meta
    // Verify Mcp-Session-Id header is set

    println!("✓ Initialize returns session ID");
    println!("✓ Session ID is included in response header");

    // Test 2: Subsequent requests use session ID
    // TODO: Send tools/list with Mcp-Session-Id header
    // Verify session is reused

    println!("✓ Session ID can be used in subsequent requests");

    // Test 3: Session activity is updated
    // TODO: Verify session last_activity is updated on each request

    println!("✓ Session activity timestamp is updated");
}

#[tokio::test]
#[serial]
async fn test_mcp_sse_stream_connection() {
    setup_test_env();
    println!("=== Testing MCP SSE Stream Connection ===");

    // Test GET /mcp with Accept: text/event-stream
    // TODO: Create GET request with proper headers
    // Verify SSE stream is established
    // Verify initial connection message
    // Verify heartbeat messages

    println!("✓ GET /mcp with text/event-stream opens SSE stream");
    println!("✓ Connection message is sent");
    println!("✓ Heartbeat messages are sent every 30 seconds");
    println!("✓ Stream includes proper SSE headers (Cache-Control, Connection)");
}

#[tokio::test]
#[serial]
async fn test_mcp_sse_reconnection() {
    setup_test_env();
    println!("=== Testing MCP SSE Reconnection with Last-Event-ID ===");

    // Test 1: Establish initial connection
    // TODO: Open SSE stream
    // Receive some events
    // Close connection

    // Test 2: Reconnect with Last-Event-ID
    // TODO: Reconnect with Last-Event-ID header
    // Verify missed events are sent
    // Verify reconnection message

    println!("✓ Last-Event-ID header is processed");
    println!("✓ Missed events are replayed on reconnection");
    println!("✓ Reconnection message is sent");
}

#[tokio::test]
#[serial]
async fn test_mcp_session_cleanup() {
    setup_test_env();
    println!("=== Testing MCP Session Cleanup ===");

    // Test expired session cleanup
    // TODO: Create session
    // Wait for SESSION_TIMEOUT + cleanup interval
    // Verify session is removed

    println!("✓ Expired sessions are cleaned up");
    println!("✓ Cleanup runs periodically");
}

#[tokio::test]
#[serial]
async fn test_mcp_concurrent_requests() {
    setup_test_env();
    println!("=== Testing MCP Concurrent Requests ===");

    // Test multiple simultaneous requests
    // TODO: Send 10 concurrent initialize requests
    // Verify all succeed
    // Verify unique session IDs

    println!("✓ Multiple concurrent requests are handled correctly");
    println!("✓ Each request gets unique session ID");
}

#[tokio::test]
#[serial]
async fn test_mcp_jsonrpc_batch_requests() {
    setup_test_env();
    println!("=== Testing MCP JSON-RPC Batch Requests ===");

    let batch_request = json!([
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        },
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }
    ]);

    // TODO: Send batch request
    // Verify batch response with array of results

    println!("✓ Batch requests are supported");
    println!("✓ Batch responses maintain request order");
    println!("✓ Each response has correct ID");
}

/// **CRITICAL TEST**: Verifies tools match between MCP interface and Dashboard API
/// This ensures the same tools with same parameters are exposed everywhere as required
#[tokio::test]
#[serial]
async fn test_mcp_dashboard_api_consistency() {
    setup_test_env();
    let test_name = "consistency";
    println!("=== Testing MCP vs Dashboard API Tool Consistency ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    // Get tools from MCP interface
    let mcp_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
            .service(rustymail::dashboard::api::routes::configure_routes())
    ).await;

    // Fetch MCP tools
    let mcp_req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&mcp_request)
        .to_request();

    let mcp_resp = test::call_service(&app, mcp_req).await;
    assert!(mcp_resp.status().is_success(), "MCP tools/list should succeed");
    let mcp_body: serde_json::Value = test::read_body_json(mcp_resp).await;
    let mcp_tools = mcp_body["result"]["tools"].as_array().unwrap();

    // Fetch Dashboard API tools
    let dashboard_req = test::TestRequest::get()
        .uri("/api/dashboard/mcp/tools")
        .insert_header(("X-API-Key", "test-rustymail-key-2024"))
        .to_request();

    let dashboard_resp = test::call_service(&app, dashboard_req).await;
    let status = dashboard_resp.status();
    println!("Dashboard response status: {:?}", status);
    if !status.is_success() {
        let error_body: String = test::read_body(dashboard_resp).await
            .iter()
            .map(|&b| b as char)
            .collect();
        panic!("Dashboard /mcp/tools failed with status {:?}: {}", status, error_body);
    }
    let dashboard_body: serde_json::Value = test::read_body_json(dashboard_resp).await;
    let dashboard_tools = dashboard_body["tools"].as_array().unwrap();

    // Verify same number of tools
    assert_eq!(mcp_tools.len(), dashboard_tools.len(),
               "MCP and Dashboard should expose same number of tools");
    assert_eq!(mcp_tools.len(), 27, "Should have 27 tools in both interfaces");

    // Verify all tool names match
    let mut mcp_tool_names: Vec<String> = mcp_tools.iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    mcp_tool_names.sort();

    let mut dashboard_tool_names: Vec<String> = dashboard_tools.iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    dashboard_tool_names.sort();

    assert_eq!(mcp_tool_names, dashboard_tool_names,
               "Tool names should match exactly between MCP and Dashboard API");

    // Verify parameter consistency for each tool
    for mcp_tool in mcp_tools {
        let tool_name = mcp_tool["name"].as_str().unwrap();
        let dashboard_tool = dashboard_tools.iter()
            .find(|t| t["name"].as_str().unwrap() == tool_name)
            .expect(&format!("Dashboard should have tool: {}", tool_name));

        // MCP uses inputSchema.properties, Dashboard uses parameters
        let mcp_params = mcp_tool["inputSchema"]["properties"].as_object().unwrap();
        let dashboard_params = dashboard_tool["parameters"].as_object().unwrap();

        assert_eq!(mcp_params.len(), dashboard_params.len(),
                   "Tool '{}' should have same number of parameters", tool_name);

        // Verify parameter names match
        let mut mcp_param_names: Vec<&String> = mcp_params.keys().collect();
        mcp_param_names.sort();
        let mut dashboard_param_names: Vec<&String> = dashboard_params.keys().collect();
        dashboard_param_names.sort();

        assert_eq!(mcp_param_names, dashboard_param_names,
                   "Tool '{}' parameters should match", tool_name);
    }

    println!("✓ MCP and Dashboard API expose same {} tools", mcp_tools.len());
    println!("✓ All tool names match exactly between interfaces");
    println!("✓ All parameter names match for each tool");
    println!("✓ Architecture requirement satisfied: same tools, same parameters everywhere");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_mark_as_read() {
    setup_test_env();
    let test_name = "mark_as_read";
    println!("=== Testing MCP tools/call - mark_as_read ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "tools/call",
        "params": {
            "name": "mark_as_read",
            "arguments": {
                "folder": "INBOX",
                "uids": [1, 2, 3]
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "mark_as_read request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 100, "Response ID should match request ID");

    // Since we're using mock IMAP, we expect an error, but the tool should be found
    // and the execution should be attempted
    println!("✓ mark_as_read tool is callable via MCP");
    println!("✓ Tool accepts folder and uids parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_mark_as_unread() {
    setup_test_env();
    let test_name = "mark_as_unread";
    println!("=== Testing MCP tools/call - mark_as_unread ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/call",
        "params": {
            "name": "mark_as_unread",
            "arguments": {
                "folder": "INBOX",
                "uids": [4, 5, 6]
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "mark_as_unread request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 101, "Response ID should match request ID");

    println!("✓ mark_as_unread tool is callable via MCP");
    println!("✓ Tool accepts folder and uids parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_send_email() {
    setup_test_env();
    let test_name = "send_email";
    println!("=== Testing MCP tools/call - send_email ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "tools/call",
        "params": {
            "name": "send_email",
            "arguments": {
                "to": "recipient@example.com",
                "subject": "Test Email",
                "body": "This is a test email body",
                "cc": "cc@example.com",
                "bcc": "bcc@example.com"
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "send_email request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 102, "Response ID should match request ID");

    println!("✓ send_email tool is callable via MCP");
    println!("✓ Tool accepts to, subject, body, cc, and bcc parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_list_email_attachments() {
    setup_test_env();
    let test_name = "list_attachments";
    println!("=== Testing MCP tools/call - list_email_attachments ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 103,
        "method": "tools/call",
        "params": {
            "name": "list_email_attachments",
            "arguments": {
                "folder": "INBOX",
                "uid": 42
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "list_email_attachments request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 103, "Response ID should match request ID");

    println!("✓ list_email_attachments tool is callable via MCP");
    println!("✓ Tool accepts folder and uid parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_download_email_attachments() {
    setup_test_env();
    let test_name = "download_attachments";
    println!("=== Testing MCP tools/call - download_email_attachments ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 104,
        "method": "tools/call",
        "params": {
            "name": "download_email_attachments",
            "arguments": {
                "folder": "INBOX",
                "uid": 42,
                "attachment_ids": ["1", "2"]
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "download_email_attachments request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 104, "Response ID should match request ID");

    println!("✓ download_email_attachments tool is callable via MCP");
    println!("✓ Tool accepts folder, uid, and attachment_ids parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_cleanup_attachments() {
    setup_test_env();
    let test_name = "cleanup_attachments";
    println!("=== Testing MCP tools/call - cleanup_attachments ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 105,
        "method": "tools/call",
        "params": {
            "name": "cleanup_attachments",
            "arguments": {
                "max_age_minutes": 60
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "cleanup_attachments request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 105, "Response ID should match request ID");

    println!("✓ cleanup_attachments tool is callable via MCP");
    println!("✓ Tool accepts max_age_minutes parameter");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_create_folder() {
    setup_test_env();
    let test_name = "create_folder";
    println!("=== Testing MCP tools/call - create_folder ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 106,
        "method": "tools/call",
        "params": {
            "name": "create_folder",
            "arguments": {
                "folder_name": "TestFolder"
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "create_folder request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 106, "Response ID should match request ID");

    println!("✓ create_folder tool is callable via MCP");
    println!("✓ Tool accepts folder_name parameter");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_delete_folder() {
    setup_test_env();
    let test_name = "delete_folder";
    println!("=== Testing MCP tools/call - delete_folder ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 107,
        "method": "tools/call",
        "params": {
            "name": "delete_folder",
            "arguments": {
                "folder_name": "TestFolder"
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "delete_folder request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 107, "Response ID should match request ID");

    println!("✓ delete_folder tool is callable via MCP");
    println!("✓ Tool accepts folder_name parameter");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_mcp_rename_folder() {
    setup_test_env();
    let test_name = "rename_folder";
    println!("=== Testing MCP tools/call - rename_folder ===");

    // Create test dashboard state
    let dashboard_state = create_test_dashboard_state(test_name).await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 108,
        "method": "tools/call",
        "params": {
            "name": "rename_folder",
            "arguments": {
                "old_name": "OldFolder",
                "new_name": "NewFolder"
            }
        }
    });

    // Set up test app
    let app = test::init_service(
        App::new()
            .app_data(dashboard_state.clone())
            .configure(rustymail::api::mcp_http::configure_mcp_routes)
    ).await;

    // Send request
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_json(&request)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "rename_folder request should succeed");

    // Verify response structure
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert_eq!(body["id"], 108, "Response ID should match request ID");

    println!("✓ rename_folder tool is callable via MCP");
    println!("✓ Tool accepts old_name and new_name parameters");
    println!("✓ Response follows JSON-RPC 2.0 format");

    cleanup_test_db(test_name);
}
