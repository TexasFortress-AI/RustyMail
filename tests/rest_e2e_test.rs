// tests/rest_e2e_test.rs
// End-to-End tests for the REST API adapter.
// Requires a compiled rustymail binary and a running IMAP server with credentials in .env.
// Run with: cargo test --test rest_e2e_test --features integration_tests

#[cfg(all(test, feature = "integration_tests"))]
mod e2e_tests {
    use std::process::{Command, Child, Stdio};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use std::thread;
    use tokio::runtime::Runtime;
    use reqwest::{Client, StatusCode};
    use serde_json::{json, Value};
    use rustymail::config::Settings;
    use dotenvy::dotenv;
    use urlencoding::encode; // For URL encoding folder names
    use rustymail::imap::ImapClient; // Assuming ImapClient might be needed later
    use actix_web::{test, App};
    use tokio;

    const BASE_URL: &str = "http://127.0.0.1:8080"; // Default REST API address
    const STARTUP_DELAY_MS: u64 = 3000; // Allow time for server to start
    const TEST_FOLDER_A: &str = "INBOX/TestingBoxA";
    const TEST_FOLDER_B: &str = "INBOX/TestingBoxB";

    // Helper to generate unique subjects/identifiers for test emails/folders
    fn unique_id(prefix: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        format!("rustymail_e2e_{}_{}", prefix, timestamp)
    }

    // Structure to manage the server process
    struct TestServer {
        process: Option<Child>,
    }

    impl TestServer {
        fn start() -> Self {
            println!("Building rustymail binary...");
            let build_status = Command::new("cargo")
                .args(["build", "--bin", "rustymail"])
                .stdout(Stdio::null()) // Suppress build output unless debugging
                .stderr(Stdio::null())
                .status()
                .expect("Failed to execute cargo build");

            if !build_status.success() {
                panic!("Cargo build failed!");
            }
            println!("Build successful.");

            println!("Starting rustymail server process...");
            // Ensure .env is loaded for the server process
            dotenv().ok(); 
            // Get REST host/port from config to ensure we connect correctly
            // Although we default to 8080, reading config is safer
            let settings = Settings::new(None).expect("Failed to load settings for test server start");
            let rest_config = settings.rest.as_ref().expect("REST config section missing or disabled");
            if !rest_config.enabled {
                panic!("REST interface must be enabled for this test");
            }
            let listen_addr = format!("{}:{}", rest_config.host, rest_config.port);
            let base_url_from_config = format!("http://{}", listen_addr);
            // Note: We are still using BASE_URL constant, but this shows how to get it if needed.
            assert_eq!(BASE_URL, base_url_from_config, "Configured REST address differs from test constant!");

            let child = Command::new("./target/debug/rustymail")
                // Add any necessary args, e.g., force REST mode if not default
                // .args(["--adapter", "rest"])
                .stdout(Stdio::piped()) // Capture stdout/stderr for debugging if needed
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start rustymail server process");

            println!("Server process started (PID: {}). Waiting {}ms for startup...", child.id(), STARTUP_DELAY_MS);
            thread::sleep(Duration::from_millis(STARTUP_DELAY_MS));

            TestServer { process: Some(child) }
        }
    }

    // Implement Drop to ensure the server process is killed when TestServer goes out of scope
    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(mut process) = self.process.take() {
                println!("Attempting to kill server process (PID: {})...", process.id());
                match process.kill() {
                    Ok(_) => {
                        // Give it a moment to exit cleanly before waiting
                        thread::sleep(Duration::from_millis(100)); 
                        match process.wait() {
                            Ok(status) => println!("Server process exited with status: {}", status),
                            Err(e) => eprintln!("Error waiting for server process exit: {}", e),
                        }
                        println!("Server process killed successfully.");
                    }
                    Err(e) => {
                        eprintln!("Failed to kill server process (PID: {}): {}", process.id(), e);
                        // Attempt to wait anyway, it might have exited already
                        if let Err(wait_e) = process.wait() {
                            eprintln!("Error waiting for server process after kill failure: {}", wait_e);
                        }
                    }
                }
            } else {
                println!("Server process already stopped or not started.");
            }
        }
    }

    // --- Test Functions ---
    // Note: These run sequentially because they share the TestServer setup.
    // Using a framework like serial_test or explicit locking would be needed for parallel execution.

    #[tokio::test]
    async fn run_rest_e2e_tests() {
        let _server = TestServer::start(); // Start server, will be killed on drop
        let client = Client::new();

        // Run async tests sequentially
        println!("\n--- Running E2E: Basic Folder Operations ---");
        test_e2e_list_folders(&client).await;
        test_e2e_get_emails_in_folder(&client, "INBOX").await; // Check INBOX first
        let created_folder_name = test_e2e_create_folder(&client).await;
        test_e2e_rename_folder(&client, &created_folder_name).await; // Rename the created folder
        // Deletion is implicitly tested by cleanup in other tests/Drop

        println!("\n--- Running E2E: Email Operations ---");
        // Append an email to use for search/move/fetch
        let unique_subject = unique_id("email_ops");
        let appended_uid = test_e2e_append_email(&client, TEST_FOLDER_A, &unique_subject).await;
        test_e2e_search_email(&client, TEST_FOLDER_A, &unique_subject, appended_uid).await;
        test_e2e_move_email(&client, TEST_FOLDER_A, TEST_FOLDER_B, appended_uid, &unique_subject).await;
        test_e2e_fetch_single_email(&client, TEST_FOLDER_B, appended_uid, &unique_subject).await;

        println!("\n--- Running E2E: Error Conditions ---");
        test_e2e_fetch_non_existent_folder(&client).await;
        test_e2e_fetch_non_existent_uid(&client).await;
        test_e2e_move_invalid_uid(&client).await;
        test_e2e_move_invalid_source(&client).await;

        println!("E2E tests completed. Server will be stopped.");
    }

    async fn test_e2e_list_folders(client: &Client) {
        println!("Testing GET /folders...");
        let res = client.get(format!("{}/folders", BASE_URL)).send().await.expect("Request failed");
        assert!(res.status().is_success(), "GET /folders failed with status: {}", res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert!(body["folders"].is_array(), "Response should contain a 'folders' array");
        // Check if known folders exist (case-insensitive check might be better)
        let folders = body["folders"].as_array().unwrap();
        assert!(folders.iter().any(|v| v.as_str().unwrap_or("").eq_ignore_ascii_case("INBOX")), "INBOX not found");
        assert!(folders.iter().any(|v| v.as_str().unwrap_or("").eq_ignore_ascii_case(TEST_FOLDER_A)), "{} not found", TEST_FOLDER_A);
        assert!(folders.iter().any(|v| v.as_str().unwrap_or("").eq_ignore_ascii_case(TEST_FOLDER_B)), "{} not found", TEST_FOLDER_B);
        println!("GET /folders OK");
    }

    async fn test_e2e_get_emails_in_folder(client: &Client, folder: &str) {
        println!("Testing GET /emails/{}...", folder);
        let url_encoded_folder = encode(folder); // Encode folder name for URL
        let res = client.get(format!("{}/emails/{}", BASE_URL, url_encoded_folder)).send().await.expect("Request failed");
        assert!(res.status().is_success(), "GET /emails/{} failed with status: {}", folder, res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert!(body["emails"].is_array(), "Response should contain an 'emails' array");
        println!("GET /emails/{} OK ({} emails found)", folder, body["emails"].as_array().unwrap().len());
    }

    // Appends an email and returns the UID assigned by the server
    async fn test_e2e_append_email(client: &Client, folder: &str, subject: &str) -> u32 {
        println!("Testing POST /emails/{} (append)...", folder);
         let url_encoded_folder = encode(folder);
        let email_data = json!({
            "subject": subject,
            "body": { "text_plain": "E2E test email body" },
            "to": ["test@example.com"]
        });
        let res = client.post(format!("{}/emails/{}", BASE_URL, url_encoded_folder))
            .json(&email_data)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(res.status(), StatusCode::CREATED, "POST /emails/{} failed with status: {}", folder, res.status()); 
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert!(body["uid"].is_number(), "Response should contain a numeric 'uid'");
        let uid = body["uid"].as_u64().expect("Failed to parse UID as u64") as u32;
        println!("POST /emails/{} OK, UID: {}", folder, uid);
        uid
    }

    async fn test_e2e_move_email(client: &Client, source: &str, dest: &str, uid: u32, expected_subject: &str) {
        println!("Testing POST /emails/move ({} -> {})...", source, dest);
        let move_data = json!({
            "source_folder": source,
            "destination_folder": dest,
            "uids": [uid]
        });
        let res = client.post(format!("{}/emails/move", BASE_URL))
            .json(&move_data)
            .send()
            .await
            .expect("Request failed");

        assert!(res.status().is_success(), "POST /emails/move failed with status: {}", res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert!(body["success_uids"].is_array() && body["success_uids"].as_array().unwrap().len() == 1, "Move success_uids incorrect");
        assert_eq!(body["success_uids"].as_array().unwrap()[0].as_u64().unwrap() as u32, uid, "Moved UID mismatch");

        // Verification: Check source folder no longer contains the email
        println!("Verifying email (UID {}) gone from source {}...", uid, source);
        let url_encoded_source = encode(source);
        let src_res = client.get(format!("{}/emails/{}", BASE_URL, url_encoded_source)).send().await.expect("Request failed");
        let src_body: Value = src_res.json().await.expect("Failed to parse JSON");
        assert!(!src_body["emails"].as_array().unwrap().iter().any(|e| e["uid"].as_u64().unwrap() as u32 == uid), "Email UID {} still found in source folder {}", uid, source);
        println!("Verification OK: Email gone from source.");

        // Verification: Check destination folder contains the email
        println!("Verifying email (Subject '{}') present in destination {}...", expected_subject, dest);
        let url_encoded_dest = encode(dest);
        let dest_res = client.get(format!("{}/emails/{}", BASE_URL, url_encoded_dest)).send().await.expect("Request failed");
        let dest_body: Value = dest_res.json().await.expect("Failed to parse JSON");
        assert!(dest_body["emails"].as_array().unwrap().iter().any(|e| e["envelope"]["subject"].as_str().unwrap_or("") == expected_subject), "Email with subject '{}' not found in destination folder {}", expected_subject, dest);
         println!("Verification OK: Email found in destination.");

        println!("POST /emails/move OK");
    }

    async fn test_e2e_fetch_single_email(client: &Client, folder: &str, uid: u32, expected_subject: &str) {
        println!("Testing GET /emails/{}/{}...", folder, uid);
        let url_encoded_folder = encode(folder);
        let res = client.get(format!("{}/emails/{}/{}", BASE_URL, url_encoded_folder, uid)).send().await.expect("Request failed");
        assert!(res.status().is_success(), "GET /emails/{}/{} failed with status: {}", folder, uid, res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert_eq!(body["uid"].as_u64().unwrap() as u32, uid, "Fetched UID mismatch");
        assert_eq!(body["envelope"]["subject"].as_str().unwrap_or(""), expected_subject, "Fetched subject mismatch");
        assert!(body["body"].is_string() || body["body"].is_object(), "Fetched email should have a body (string or object)");
        println!("GET /emails/{}/{} OK", folder, uid);
    }
    
    // --- Folder Management Tests ---

    async fn test_e2e_create_folder(client: &Client) -> String {
        let folder_name = unique_id("create_folder");
        println!("Testing POST /folders (create {})...", folder_name);
        let create_data = json!({ "name": folder_name });
        let res = client.post(format!("{}/folders", BASE_URL))
            .json(&create_data)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(res.status(), StatusCode::CREATED, "POST /folders failed with status: {}", res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert_eq!(body["name"].as_str().unwrap(), folder_name, "Created folder name mismatch");

        // Verify folder exists in list
        let list_res = client.get(format!("{}/folders", BASE_URL)).send().await.expect("List request failed");
        let list_body: Value = list_res.json().await.expect("Failed to parse list JSON");
        assert!(list_body["folders"].as_array().unwrap().iter().any(|v| v.as_str().unwrap_or("") == folder_name), "Newly created folder '{}' not found in list", folder_name);
        
        println!("POST /folders OK");
        folder_name // Return name for potential cleanup or subsequent tests
    }

    // Note: Deletion is handled implicitly by rename cleanup in the current flow
    async fn test_e2e_delete_folder(client: &Client, folder_name: &str) {
         println!("Testing DELETE /folders/{}...", folder_name);
         let url_encoded_folder = encode(folder_name);
         let res = client.delete(format!("{}/folders/{}", BASE_URL, url_encoded_folder))
             .send()
             .await
             .expect("Request failed");
        
         assert!(res.status().is_success(), "DELETE /folders/{} failed with status: {}", folder_name, res.status());
         let body: Value = res.json().await.expect("Failed to parse JSON response");
         assert_eq!(body["name"].as_str().unwrap(), folder_name, "Deleted folder name mismatch");

         // Verify folder is gone from list
         let list_res = client.get(format!("{}/folders", BASE_URL)).send().await.expect("List request failed");
         let list_body: Value = list_res.json().await.expect("Failed to parse list JSON");
         assert!(!list_body["folders"].as_array().unwrap().iter().any(|v| v.as_str().unwrap_or("") == folder_name), "Deleted folder '{}' still found in list", folder_name);
        
         println!("DELETE /folders/{} OK", folder_name);
     }

    async fn test_e2e_rename_folder(client: &Client, old_name: &str) {
        let new_name = unique_id("renamed_folder");
        println!("Testing PUT /folders/{} (rename to {})...", old_name, new_name);
        let rename_data = json!({ "new_name": new_name });
        let url_encoded_old_name = encode(old_name);
        let res = client.put(format!("{}/folders/{}", BASE_URL, url_encoded_old_name))
            .json(&rename_data)
            .send()
            .await
            .expect("Request failed");

        assert!(res.status().is_success(), "PUT /folders/{} failed with status: {}", old_name, res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        assert_eq!(body["old_name"].as_str().unwrap(), old_name, "Rename old_name mismatch");
        assert_eq!(body["new_name"].as_str().unwrap(), new_name, "Rename new_name mismatch");

        // Verify rename in list
        let list_res = client.get(format!("{}/folders", BASE_URL)).send().await.expect("List request failed");
        let list_body: Value = list_res.json().await.expect("Failed to parse list JSON");
        let folders = list_body["folders"].as_array().unwrap();
        assert!(!folders.iter().any(|v| v.as_str().unwrap_or("") == old_name), "Old folder name '{}' still found after rename", old_name);
        assert!(folders.iter().any(|v| v.as_str().unwrap_or("") == new_name), "New folder name '{}' not found after rename", new_name);

        println!("PUT /folders/{} OK", old_name);

        // Cleanup: Delete the renamed folder
        test_e2e_delete_folder(client, &new_name).await;
    }
    
    // --- Search Test ---
    async fn test_e2e_search_email(client: &Client, folder: &str, subject: &str, expected_uid: u32) {
        println!("Testing GET /emails/{}/search (subject '{}')...", folder, subject);
        let url_encoded_folder = encode(folder);
        let url_encoded_subject = encode(subject);
        let search_url = format!(
            "{}/emails/{}/search?criteria=subject&value={}",
            BASE_URL, url_encoded_folder, url_encoded_subject
        );
        let res = client.get(&search_url).send().await.expect("Request failed");

        assert!(res.status().is_success(), "GET search failed with status: {}", res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        assert!(body["emails"].is_array(), "Search response missing 'emails' array");
        let emails = body["emails"].as_array().unwrap();
        assert_eq!(emails.len(), 1, "Expected exactly one email from subject search");
        assert_eq!(emails[0]["uid"].as_u64().unwrap() as u32, expected_uid, "Search returned wrong UID");
        assert_eq!(emails[0]["envelope"]["subject"].as_str().unwrap_or(""), subject, "Search returned wrong subject");

        println!("GET /emails/{}/search OK", folder);
    }

    // --- Error Condition Tests ---

    async fn test_e2e_fetch_non_existent_folder(client: &Client) {
        let folder = unique_id("nonexistent_folder");
        println!("Testing GET /emails/{} (non-existent)...", folder);
         let url_encoded_folder = encode(&folder);
        let res = client.get(format!("{}/emails/{}", BASE_URL, url_encoded_folder)).send().await.expect("Request failed");
        assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND, "Expected 404 for non-existent folder, got {}", res.status());
        println!("GET /emails/{} (non-existent) OK (404)", folder);
    }

    async fn test_e2e_fetch_non_existent_uid(client: &Client) {
        let uid = 999_999_999u32;
        // Use a known good folder like TEST_FOLDER_A
        println!("Testing GET /emails/{}/{} (non-existent UID)...", TEST_FOLDER_A, uid);
         let url_encoded_folder = encode(TEST_FOLDER_A);
        let res = client.get(format!("{}/emails/{}/{}", BASE_URL, url_encoded_folder, uid)).send().await.expect("Request failed");
        assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND, "Expected 404 for non-existent UID, got {}", res.status());
        println!("GET /emails/{}/{} (non-existent UID) OK (404)", TEST_FOLDER_A, uid);
    }
    
    async fn test_e2e_move_invalid_uid(client: &Client) {
        let invalid_uid = 999_999_998u32; // Use a different invalid UID
        println!("Testing POST /emails/move (invalid UID {})...", invalid_uid);
        let move_data = json!({
            "source_folder": TEST_FOLDER_A,
            "destination_folder": TEST_FOLDER_B,
            "uids": [invalid_uid]
        });
        let res = client.post(format!("{}/emails/move", BASE_URL))
            .json(&move_data)
            .send()
            .await
            .expect("Request failed");
            
        // Expect success overall, but the UID should be in failed_uids
        assert!(res.status().is_success(), "POST /emails/move (invalid UID) failed with status: {}", res.status());
        let body: Value = res.json().await.expect("Failed to parse JSON response");
        println!("Response: {}", body);
        assert!(body["success_uids"].as_array().unwrap().is_empty(), "Expected no success_uids for invalid UID move");
        assert!(body["failed_uids"].is_array() && body["failed_uids"].as_array().unwrap().len() == 1, "Expected one failed_uid for invalid UID move");
        assert_eq!(body["failed_uids"].as_array().unwrap()[0]["uid"].as_u64().unwrap() as u32, invalid_uid, "failed_uid mismatch");
        // Optionally check error message: body["failed_uids"][0]["error"] 

        println!("POST /emails/move (invalid UID) OK (UID listed in failed_uids)");
    }

    async fn test_e2e_move_invalid_source(client: &Client) {
        let source_folder = unique_id("nonexistent_source");
        // Use a known valid UID from previous tests if possible, or append a temp one?
        // For simplicity, let's just use a plausible UID, the source folder check happens first.
        let plausible_uid = 1u32; 
        println!("Testing POST /emails/move (invalid source {})...", source_folder);
        let move_data = json!({
            "source_folder": source_folder,
            "destination_folder": TEST_FOLDER_B,
            "uids": [plausible_uid]
        });
        let res = client.post(format!("{}/emails/move", BASE_URL))
            .json(&move_data)
            .send()
            .await
            .expect("Request failed");
            
        // Expect a 404 Not Found because the source folder doesn't exist
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "Expected 404 for move from non-existent source, got {}", res.status());
        
        println!("POST /emails/move (invalid source) OK (404)");
    }

    // TODO: Add E2E tests for:
    // - More folder error conditions (delete non-empty, rename to existing)

} 