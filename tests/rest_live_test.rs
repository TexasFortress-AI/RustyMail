// tests/rest_live_test.rs
#[cfg(all(test, feature = "live_tests"))] // Only run if feature is enabled
mod live_tests {
    use actix_web::{test, web, App, http::StatusCode};
    use rustymail::{
        api::rest::{configure_rest_service, AppState},
        imap::{client::ImapClient, types::Folder, types::MailboxInfo, types::SearchCriteria, types::ModifyFlagsPayload, types::AppendEmailPayload, types::Email},
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

    #[actix_web::test]
    async fn test_live_select_folder() {
        let (mut app, _client) = setup_test_app_live().await;
        let folder_name = "INBOX"; // Select a standard folder
        let encoded_name = urlencoding::encode(folder_name);

        println!("Live Test: Selecting folder '{}'", folder_name);

        let select_req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/select", encoded_name))
            .to_request();
        let select_resp = test::call_service(&mut app, select_req).await;

        assert!(select_resp.status().is_success(), "Select API call failed with status: {}", select_resp.status());
        let mailbox_info: MailboxInfo = test::read_body_json(select_resp).await;

        println!("Select result: {:?}", mailbox_info);
        assert!(mailbox_info.exists > 0, "Expected INBOX to have existing emails");
    }

    #[actix_web::test]
    async fn test_live_search_emails() {
        let (mut app, _client) = setup_test_app_live().await;
        let folder_name = "INBOX"; 
        let encoded_folder_name = urlencoding::encode(folder_name);
        // Simple search for all emails
        let search_query = "ALL";
        let encoded_query = urlencoding::encode(search_query);

        println!("Live Test: Searching folder '{}' with query '{}'", folder_name, search_query);

        let search_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails/search?query={}", encoded_folder_name, encoded_query))
            .to_request();
        let search_resp = test::call_service(&mut app, search_req).await;

        assert!(search_resp.status().is_success(), "Search API call failed with status: {}", search_resp.status());
        let uids: Vec<u32> = test::read_body_json(search_resp).await;

        println!("Search result UIDs: {:?}", uids);
        assert!(!uids.is_empty(), "Expected search query '{}' in folder '{}' to return some UIDs", search_query, folder_name);
    }

    #[actix_web::test]
    async fn test_live_fetch_emails() {
        let (mut app, _client) = setup_test_app_live().await;
        let folder_name = "INBOX";
        let encoded_folder_name = urlencoding::encode(folder_name);

        // First, find some UIDs to fetch (reuse search logic)
        let search_query = "ALL";
        let encoded_query = urlencoding::encode(search_query);
        let search_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails/search?query={}", encoded_folder_name, encoded_query))
            .to_request();
        let search_resp = test::call_service(&mut app, search_req).await;
        let uids: Vec<u32> = test::read_body_json(search_resp).await;
        assert!(!uids.is_empty(), "Need UIDs from search to run fetch test");
        let uids_to_fetch = uids.iter().take(2).cloned().collect::<Vec<u32>>(); // Fetch first 2
        let uids_param = uids_to_fetch.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");

        println!("Live Test: Fetching UIDs '{}' from folder '{}'", uids_param, folder_name);

        // Fetch without body first
        let fetch_req_no_body = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}", encoded_folder_name, uids_param))
            .to_request();
        let fetch_resp_no_body = test::call_service(&mut app, fetch_req_no_body).await;
        assert!(fetch_resp_no_body.status().is_success(), "Fetch (no body) API call failed: {}", fetch_resp_no_body.status());
        let emails_no_body: Vec<Email> = test::read_body_json(fetch_resp_no_body).await;
        println!("Fetch (no body) result count: {}", emails_no_body.len()); // Log count for clarity
        assert_eq!(emails_no_body.len(), uids_to_fetch.len());
        assert!(emails_no_body.iter().all(|e| e.body.is_none()), "Expected no bodies when fetchBody=false");
        assert!(emails_no_body.iter().all(|e| uids_to_fetch.contains(&e.uid)), "Fetched UIDs don't match requested");

        // Fetch *with* body
        println!("Live Test: Fetching UIDs '{}' WITH BODY from folder '{}'", uids_param, folder_name);
        let fetch_req_with_body = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}&fetchBody=true", encoded_folder_name, uids_param))
            .to_request();
        let fetch_resp_with_body = test::call_service(&mut app, fetch_req_with_body).await;
        assert!(fetch_resp_with_body.status().is_success(), "Fetch (with body) API call failed: {}", fetch_resp_with_body.status());
        let emails_with_body: Vec<Email> = test::read_body_json(fetch_resp_with_body).await;
        println!("Fetch (with body) result count: {}", emails_with_body.len());
        assert_eq!(emails_with_body.len(), uids_to_fetch.len());
        // Check if the body field is present and not empty
        assert!(emails_with_body.iter().all(|e| e.body.is_some() && !e.body.as_ref().unwrap().is_empty()), "Expected non-empty bodies when fetchBody=true");
        assert!(emails_with_body.iter().all(|e| uids_to_fetch.contains(&e.uid)), "Fetched UIDs don't match requested");
    }

    #[actix_web::test]
    async fn test_live_move_email() {
        let (mut app, client) = setup_test_app_live().await;
        let source_folder = "INBOX";
        let dest_base_folder = "LiveTestMoveDest";
        let dest_full_folder = format!("INBOX.{}", dest_base_folder);
        let encoded_source_folder = urlencoding::encode(source_folder);
        // Note: The API expects the BASE destination name in the payload

        println!("Live Test: Setting up for move from '{}' to '{}'", source_folder, dest_base_folder);

        // 1. Ensure destination folder exists (and cleanup if needed)
        let _ = client.delete_folder(&dest_full_folder).await; // Cleanup previous run
        let create_res = client.create_folder(&dest_full_folder).await;
        assert!(create_res.is_ok(), "Failed to create destination folder '{}' for move test", dest_full_folder);

        // 2. Get a UID from the source folder (INBOX)
        let search_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails/search?query=ALL", encoded_source_folder))
            .to_request();
        let search_resp = test::call_service(&mut app, search_req).await;
        let uids: Vec<u32> = test::read_body_json(search_resp).await;
        assert!(!uids.is_empty(), "INBOX must have emails to test move");
        let uid_to_move = uids[0]; // Move the first email found
        println!("Live Test: Attempting to move UID {} from {} to {}", uid_to_move, source_folder, dest_base_folder);

        // 3. Perform the move via API
        let move_req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/emails/move", encoded_source_folder))
            .set_json(&serde_json::json!({
                "uids": [uid_to_move],
                "destination_folder": dest_base_folder
            }))
            .to_request();
        let move_resp = test::call_service(&mut app, move_req).await;
        assert!(move_resp.status().is_success(), "Move API call failed: {}", move_resp.status());

        // 4. Verify the move (simple check: try to fetch from original folder - should fail or not be found)
        // A more robust check would involve searching both folders or checking counts
        let fetch_req_after_move = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}", encoded_source_folder, uid_to_move))
            .to_request();
        let fetch_resp_after_move = test::call_service(&mut app, fetch_req_after_move).await;
        // Depending on server behavior, this might be 404 or 200 with empty list
        if fetch_resp_after_move.status().is_success() {
            let emails_after_move: Vec<Email> = test::read_body_json(fetch_resp_after_move).await;
            assert!(emails_after_move.is_empty(), "Email UID {} should not be found in {} after move", uid_to_move, source_folder);
        } else {
             println!("Fetch after move returned non-success (expected if UID gone): {}", fetch_resp_after_move.status());
        }

        // Optional: Verify email exists in destination folder (more complex search needed)

        // 5. Cleanup destination folder
        println!("Live Test: Cleaning up destination folder '{}'", dest_full_folder);
        let _ = client.delete_folder(&dest_full_folder).await;
    }

    #[actix_web::test]
    async fn test_live_flags_operations() {
        let (mut app, _client) = setup_test_app_live().await;
        let folder_name = "INBOX";
        let encoded_folder = urlencoding::encode(folder_name);

        // Search for some emails
        let search_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails/search?query=ALL", encoded_folder))
            .to_request();
        let search_resp = test::call_service(&mut app, search_req).await;
        assert!(search_resp.status().is_success(), "Search failed");
        let uids: Vec<u32> = test::read_body_json(search_resp).await;
        assert!(!uids.is_empty(), "No emails found to test flags");
        let uid = uids[0];

        // Add \Flagged flag
        let add_req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/emails/flags", encoded_folder))
            .set_json(&serde_json::json!({
                "uids": [uid],
                "operation": "Add",
                "flags": ["\\Flagged"]
            }))
            .to_request();
        let add_resp = test::call_service(&mut app, add_req).await;
        assert!(add_resp.status().is_success(), "Add flag failed");

        // Fetch email and verify flag present
        let fetch_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}", encoded_folder, uid))
            .to_request();
        let fetch_resp = test::call_service(&mut app, fetch_req).await;
        assert!(fetch_resp.status().is_success(), "Fetch after add failed");
        let emails: Vec<Email> = test::read_body_json(fetch_resp).await;
        assert_eq!(emails.len(), 1);
        assert!(emails[0].flags.contains(&"\\Flagged".to_string()), "Flag not added");

        // Remove \Flagged flag
        let remove_req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/emails/flags", encoded_folder))
            .set_json(&serde_json::json!({
                "uids": [uid],
                "operation": "Remove",
                "flags": ["\\Flagged"]
            }))
            .to_request();
        let remove_resp = test::call_service(&mut app, remove_req).await;
        assert!(remove_resp.status().is_success(), "Remove flag failed");

        // Fetch email and verify flag removed
        let fetch_req2 = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}", encoded_folder, uid))
            .to_request();
        let fetch_resp2 = test::call_service(&mut app, fetch_req2).await;
        assert!(fetch_resp2.status().is_success(), "Fetch after remove failed");
        let emails2: Vec<Email> = test::read_body_json(fetch_resp2).await;
        assert_eq!(emails2.len(), 1);
        assert!(!emails2[0].flags.contains(&"\\Flagged".to_string()), "Flag not removed");
    }

    #[actix_web::test]
    async fn test_live_append_email() {
        let (mut app, _client) = setup_test_app_live().await;
        let folder_name = "INBOX";
        let encoded_folder = urlencoding::encode(folder_name);

        // Compose unique subject
        let unique_subject = format!("LiveTestAppend_{}", chrono::Utc::now().timestamp());

        // Append email
        let append_req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/emails/append", encoded_folder))
            .set_json(&serde_json::json!({
                "subject": unique_subject,
                "body": "This is a test email body.",
                "from": "test@example.com",
                "to": ["test@example.com"]
            }))
            .to_request();
        let append_resp = test::call_service(&mut app, append_req).await;
        assert!(append_resp.status().is_success(), "Append email failed");

        // Search for the appended email by subject
        let encoded_query = urlencoding::encode(&format!("SUBJECT \"{}\"", unique_subject));
        let search_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails/search?query={}", encoded_folder, encoded_query))
            .to_request();
        let search_resp = test::call_service(&mut app, search_req).await;
        assert!(search_resp.status().is_success(), "Search after append failed");
        let uids: Vec<u32> = test::read_body_json(search_resp).await;
        assert!(!uids.is_empty(), "Appended email not found in search");

        // Fetch the appended email
        let uids_param = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let fetch_req = test::TestRequest::get()
            .uri(&format!("/api/v1/folders/{}/emails?uids={}&fetchBody=true", encoded_folder, uids_param))
            .to_request();
        let fetch_resp = test::call_service(&mut app, fetch_req).await;
        assert!(fetch_resp.status().is_success(), "Fetch appended email failed");
        let emails: Vec<Email> = test::read_body_json(fetch_resp).await;
        assert!(!emails.is_empty(), "No emails fetched after append");
        assert!(emails.iter().any(|e| e.body.as_deref() == Some("This is a test email body.")), "Appended email body mismatch");
    }
}
