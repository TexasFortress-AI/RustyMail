// tests/rest_live_test.rs
#[cfg(all(test, feature = "live_tests"))] // Only run if feature is enabled
mod live_tests {
    use actix_web::{test, web, App, http::StatusCode};
    use rustymail::{
        api::rest::{configure_rest_service, AppState},
        config::Settings,
        imap::{client::ImapClient, types::Folder},
    };
    use std::sync::Arc;
    use serde_json::json;
    use urlencoding; // Needed for create/delete test
    use actix_web::dev::{Service, ServiceResponse};
    use actix_web::Error as ActixError;
    use actix_http::Request;
    use env_logger; // Add import for env_logger
    use dotenv; // Add import for dotenv

    // --- Test Setup Helper ---

    // Remove Lazy Static setup
    /*
    static TEST_SETUP: Lazy<(...)> = Lazy::new(|| {
        ...
    });
    fn get_test_service() -> ... { ... }
    fn get_test_client() -> ... { ... }
    */

     // Setup function - creates service and live client per test
     async fn setup_test_app_live() -> (impl Service<Request, Response = ServiceResponse, Error = ActixError>, Arc<ImapClient>) {
        // Ensure logging is initialized for tests
        let _ = env_logger::builder().is_test(true).try_init();

        // Load .env file into the environment for this test process
        dotenv::dotenv().ok(); // Use dotenv crate

        // Read IMAP connection details directly from environment variables
        let imap_host = std::env::var("IMAP_HOST").expect("Missing IMAP_HOST env var");
        let imap_port_str = std::env::var("IMAP_PORT").expect("Missing IMAP_PORT env var");
        let imap_port: u16 = imap_port_str.parse().expect("Invalid IMAP_PORT format");
        let imap_user = std::env::var("IMAP_USER").expect("Missing IMAP_USER env var");
        let imap_pass = std::env::var("IMAP_PASS").expect("Missing IMAP_PASS env var");

        println!(
            "Connecting to live test IMAP server at {}:{} for test...",
            imap_host, imap_port
        );
        let imap_client = ImapClient::connect(
                &imap_host, imap_port, &imap_user, &imap_pass
            ).await.expect("Failed to connect");
        let shared_client = Arc::new(imap_client);
        let app_state = AppState { imap_client: shared_client.clone() };
        // Initialize service within the test setup function
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .configure(configure_rest_service)
            ).await;
        (app, shared_client)
    }

    // --- Test Cases ---

    #[actix_web::test]
    async fn test_live_health_check() {
        let (mut app, _) = setup_test_app_live().await; // Use per-test setup
        let req = test::TestRequest::get().uri("/api/v1/health").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body, json!({ "status": "OK" }));
    }

    #[actix_web::test]
    async fn test_live_list_folders() {
        let (mut app, _) = setup_test_app_live().await; // Use per-test setup
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let folders: Vec<Folder> = test::read_body_json(resp).await;
        println!("Live folders found: {:?}", folders);

        // Assert that default folders from GreenMail exist
        assert!(folders.iter().any(|f| f.name == "INBOX"));
    }

     #[actix_web::test]
    async fn test_live_create_and_delete_folder() {
        let (mut app, client) = setup_test_app_live().await; // Use per-test setup
        let base_folder_name = "LiveTestDeleteMe";
        // We expect the API to handle prefixing, but the actual name includes it
        let full_folder_name = format!("INBOX.{}", base_folder_name);
        let encoded_name = urlencoding::encode(base_folder_name); // API uses base name

        // Ensure folder doesn't exist initially (using the live client with full name)
        // We need to delete INBOX.LiveTestDeleteMe
        let _ = client.delete_folder(&full_folder_name).await;

        // 1. Create Folder via API (using base name)
        let create_req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&serde_json::json!({ "name": base_folder_name }))
            .to_request();
        let create_resp = test::call_service(&mut app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::CREATED);

        // 2. Verify folder exists (using list folders API call)
         let list_req = test::TestRequest::get().uri("/api/v1/folders").to_request();
         let list_resp = test::call_service(&mut app, list_req).await;
         assert_eq!(list_resp.status(), StatusCode::OK);
         let folders: Vec<Folder> = test::read_body_json(list_resp).await;
         // Assert that the FULL name exists in the list
         assert!(folders.iter().any(|f| f.name == full_folder_name), "Folder '{}' was not created", full_folder_name);

        // 3. Delete Folder via API (using base name in URL)
        let delete_req = test::TestRequest::delete()
            .uri(&format!("/api/v1/folders/{}", encoded_name))
            .to_request();
        let delete_resp = test::call_service(&mut app, delete_req).await;
        assert_eq!(delete_resp.status(), StatusCode::OK);

         // 4. Verify folder is gone (using list folders API call)
         let list_req_after = test::TestRequest::get().uri("/api/v1/folders").to_request();
         let list_resp_after = test::call_service(&mut app, list_req_after).await;
         assert_eq!(list_resp_after.status(), StatusCode::OK);
         let folders_after: Vec<Folder> = test::read_body_json(list_resp_after).await;
         // Assert that the FULL name is no longer in the list
         assert!(!folders_after.iter().any(|f| f.name == full_folder_name), "Folder '{}' was not deleted", full_folder_name);
    }

    #[actix_web::test]
    async fn test_live_rename_folder() {
        let (mut app, client) = setup_test_app_live().await;
        let old_base_name = "LiveTestRenameFrom";
        let new_base_name = "LiveTestRenameTo";
        let old_full_name = format!("INBOX.{}", old_base_name);
        let new_full_name = format!("INBOX.{}", new_base_name);
        let encoded_old_name = urlencoding::encode(old_base_name);

        // Cleanup: Ensure folders don't exist from previous runs
        let _ = client.delete_folder(&old_full_name).await;
        let _ = client.delete_folder(&new_full_name).await;

        // 1. Create the initial folder via API
        let create_req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&serde_json::json!({ "name": old_base_name }))
            .to_request();
        let create_resp = test::call_service(&mut app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::CREATED, "Failed to create initial folder {}", old_base_name);

        // Verify initial creation
        let list_resp_before = test::TestRequest::get().uri("/api/v1/folders").send_request(&mut app).await;
        assert_eq!(list_resp_before.status(), StatusCode::OK);
        let folders_before: Vec<Folder> = test::read_body_json(list_resp_before).await;
        assert!(folders_before.iter().any(|f| f.name == old_full_name), "Folder '{}' should exist before rename", old_full_name);
        assert!(!folders_before.iter().any(|f| f.name == new_full_name), "Folder '{}' should not exist before rename", new_full_name);

        // 2. Rename the folder via API
        let rename_req = test::TestRequest::put()
            .uri(&format!("/api/v1/folders/{}", encoded_old_name))
            .set_json(&serde_json::json!({ "to_name": new_base_name }))
            .to_request();
        let rename_resp = test::call_service(&mut app, rename_req).await;
        assert_eq!(rename_resp.status(), StatusCode::OK, "Rename API call failed");

        // 3. Verify the rename (using list folders API call)
        let list_req_after = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let list_resp_after = test::call_service(&mut app, list_req_after).await;
        assert_eq!(list_resp_after.status(), StatusCode::OK);
        let folders_after: Vec<Folder> = test::read_body_json(list_resp_after).await;

        assert!(!folders_after.iter().any(|f| f.name == old_full_name), "Old folder name '{}' should not exist after rename", old_full_name);
        assert!(folders_after.iter().any(|f| f.name == new_full_name), "New folder name '{}' should exist after rename", new_full_name);

        // 4. Cleanup: Delete the renamed folder
        let _ = client.delete_folder(&new_full_name).await;
    }

}
