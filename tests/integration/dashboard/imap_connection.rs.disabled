use actix_web::{test, web, App, http::StatusCode};
use rustymail::{
    dashboard::{
        api::{handlers::*, routes::configure_routes},
        services::DashboardState,
    },
    imap::{ImapAdapter, MockAdapter},
    config::Config,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::{json, Value};
use futures_util::StreamExt;

async fn create_test_app() -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    Arc<DashboardState>,
) {
    let config = Config::from_env();
    let state = Arc::new(DashboardState::new(config).await);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(configure_routes)
    ).await;

    (app, state)
}

#[actix_web::test]
async fn test_imap_connection_lifecycle() {
    let (app, state) = create_test_app().await;

    // Test initial connection state
    let req = test::TestRequest::get()
        .uri("/api/dashboard/config")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = test::read_body_json(resp).await;
    assert!(body.get("imap_adapter").is_some());
    assert!(body.get("connection_status").is_some());
}

#[actix_web::test]
async fn test_imap_folder_operations() {
    let (app, _state) = create_test_app().await;

    // List folders
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "list_folders",
            "parameters": {}
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["success"], true);
    assert!(body["data"].is_array());

    // List hierarchical folders
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "list_folders_hierarchical",
            "parameters": {}
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_imap_search_operations() {
    let (app, _state) = create_test_app().await;

    // Search emails
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "search_emails",
            "parameters": {
                "folder": "INBOX",
                "query": "ALL",
                "max_results": 10
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));

    // Search with specific criteria
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "search_emails",
            "parameters": {
                "folder": "INBOX",
                "query": "FROM test@example.com",
                "max_results": 5
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_imap_message_operations() {
    let (app, _state) = create_test_app().await;

    // Fetch email with MIME
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "fetch_emails_with_mime",
            "parameters": {
                "folder": "INBOX",
                "uid": "1"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Move single message
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "atomic_move_message",
            "parameters": {
                "source_folder": "INBOX",
                "target_folder": "Archive",
                "uid": "1"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Batch move messages
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "atomic_batch_move",
            "parameters": {
                "source_folder": "INBOX",
                "target_folder": "Archive",
                "uids": "2,3,4,5"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_imap_delete_operations() {
    let (app, _state) = create_test_app().await;

    // Mark as deleted
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "mark_as_deleted",
            "parameters": {
                "folder": "INBOX",
                "uids": "10,11,12"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Undelete messages
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "undelete_messages",
            "parameters": {
                "folder": "INBOX",
                "uids": "10,11"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Expunge folder
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "expunge",
            "parameters": {
                "folder": "INBOX"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Permanently delete
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "delete_messages",
            "parameters": {
                "folder": "Trash",
                "uids": "100,101,102"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_imap_connection_recovery() {
    let (app, state) = create_test_app().await;

    // Simulate connection failure
    state.email_service.simulate_disconnect().await;

    // Try operation (should attempt reconnect)
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "list_folders",
            "parameters": {}
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should either succeed (reconnected) or return appropriate error
    assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::SERVICE_UNAVAILABLE);

    // Verify connection state
    let req = test::TestRequest::get()
        .uri("/api/dashboard/stats")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = test::read_body_json(resp).await;
    assert!(body.get("imap_connection_status").is_some());
}

#[actix_web::test]
async fn test_imap_concurrent_operations() {
    let (app, _state) = create_test_app().await;

    // Launch multiple concurrent operations
    let mut handles = vec![];

    for i in 0..5 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let req = test::TestRequest::post()
                .uri("/api/dashboard/mcp/execute")
                .set_json(&json!({
                    "tool": "search_emails",
                    "parameters": {
                        "folder": "INBOX",
                        "query": format!("SUBJECT test-{}", i),
                        "max_results": 5
                    }
                }))
                .to_request();

            test::call_service(&app_clone, req).await
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        let resp = handle.await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

#[actix_web::test]
async fn test_imap_error_handling() {
    let (app, _state) = create_test_app().await;

    // Invalid folder
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "search_emails",
            "parameters": {
                "folder": "NONEXISTENT_FOLDER",
                "query": "ALL",
                "max_results": 10
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;

    // Should handle error gracefully
    assert!(body.get("success").is_some());
    if !body["success"].as_bool().unwrap() {
        assert!(body.get("error").is_some());
    }

    // Invalid UID
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "fetch_emails_with_mime",
            "parameters": {
                "folder": "INBOX",
                "uid": "invalid-uid"
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::BAD_REQUEST);

    // Missing required parameters
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "atomic_move_message",
            "parameters": {
                "source_folder": "INBOX"
                // Missing target_folder and uid
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::OK);
}

#[actix_web::test]
async fn test_imap_session_persistence() {
    let (app, state) = create_test_app().await;

    // Perform multiple operations on same session
    let operations = vec![
        ("list_folders", json!({})),
        ("search_emails", json!({
            "folder": "INBOX",
            "query": "ALL",
            "max_results": 5
        })),
        ("list_folders_hierarchical", json!({})),
    ];

    for (tool, params) in operations {
        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&json!({
                "tool": tool,
                "parameters": params
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // Verify session is still active
    assert!(state.email_service.is_connected().await);
}

#[actix_web::test]
async fn test_imap_large_result_handling() {
    let (app, _state) = create_test_app().await;

    // Search with large result set
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "search_emails",
            "parameters": {
                "folder": "INBOX",
                "query": "ALL",
                "max_results": 1000
            }
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = test::read_body_json(resp).await;

    // Should handle large results
    if body["success"].as_bool().unwrap() {
        if let Some(data) = body.get("data") {
            if let Some(messages) = data.as_array() {
                // Results should be capped at reasonable limit
                assert!(messages.len() <= 1000);
            }
        }
    }
}

#[actix_web::test]
async fn test_imap_adapter_switching() {
    let (app, state) = create_test_app().await;

    // Get current adapter
    let req = test::TestRequest::get()
        .uri("/api/dashboard/config")
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let original_adapter = body["imap_adapter"].as_str().unwrap().to_string();

    // Switch to different adapter
    state.config_service.update_adapter("gmail").await;

    // Verify switch
    let req = test::TestRequest::get()
        .uri("/api/dashboard/config")
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["imap_adapter"], "gmail");

    // Switch back
    state.config_service.update_adapter(&original_adapter).await;
}

#[actix_web::test]
async fn test_imap_authentication_handling() {
    let (app, state) = create_test_app().await;

    // Simulate auth failure
    state.email_service.simulate_auth_failure().await;

    // Try operation
    let req = test::TestRequest::post()
        .uri("/api/dashboard/mcp/execute")
        .set_json(&json!({
            "tool": "list_folders",
            "parameters": {}
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should return auth error
    if resp.status() != StatusCode::OK {
        assert!(resp.status() == StatusCode::UNAUTHORIZED ||
                resp.status() == StatusCode::FORBIDDEN);
    } else {
        let body: Value = test::read_body_json(resp).await;
        assert!(!body["success"].as_bool().unwrap());
        assert!(body.get("error").is_some());
    }
}

#[actix_web::test]
async fn test_imap_connection_pooling() {
    let (app, state) = create_test_app().await;

    // Get connection pool stats
    let stats = state.email_service.get_pool_stats().await;
    let initial_connections = stats.active_connections;

    // Perform operations that should reuse connections
    for _ in 0..3 {
        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&json!({
                "tool": "list_folders",
                "parameters": {}
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // Check pool didn't create excessive connections
    let stats = state.email_service.get_pool_stats().await;
    assert!(stats.active_connections <= initial_connections + 1);
}