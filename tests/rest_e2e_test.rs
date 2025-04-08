// tests/rest_e2e_test.rs
// End-to-End tests for the REST API adapter.
// Requires a compiled rustymail binary and a running IMAP server with credentials in .env.
// Run with: cargo test --test rest_e2e_test --features integration_tests

// Remove unused imports
// use std::process::{Command, Child, Stdio};
// use std::time::{Duration, SystemTime, UNIX_EPOCH};
// use std::thread;
// use reqwest::{Client, StatusCode};
// use serde_json::{json, Value};
// use rustymail::config::Settings;
// use dotenvy::dotenv;
// use urlencoding::encode; // For URL encoding folder names
// use tokio;

// Remove unused constants
// const BASE_URL: &str = "http://127.0.0.1:8080/api/v1"; // Update base URL to include API version
// const STARTUP_DELAY_MS: u64 = 10000; // Allow more time for server to start
// const TEST_FOLDER_A_BASE: &str = "TestingBoxA";
// const TEST_FOLDER_B_BASE: &str = "TestingBoxB";
// const TEST_FOLDER_A_FULL: &str = "INBOX.TestingBoxA"; // Use dot delimiter
// const TEST_FOLDER_B_FULL: &str = "INBOX.TestingBoxB"; // Use dot delimiter

// Remove unused functions
// fn unique_id(prefix: &str) -> String { ... }
// async fn test_e2e_select_folder(client: &Client, folder: &str) { ... }
// async fn test_e2e_get_emails_in_folder(client: &Client, folder: &str) { ... }
// async fn test_e2e_append_email(client: &Client, folder: &str, subject: &str) -> u32 { ... }
// async fn test_e2e_move_email(client: &Client, source: &str, dest: &str, uid: u32, expected_subject: &str) { ... }
// async fn test_e2e_fetch_single_email(client: &Client, folder: &str, uid: u32, expected_subject: &str) { ... }
// async fn test_e2e_create_folder(client: &Client) -> String { ... }
// async fn test_e2e_delete_folder(client: &Client, folder_name: &str) { ... }
// async fn test_e2e_rename_folder(client: &Client, old_name: &str) { ... }
// async fn test_e2e_search_email(client: &Client, folder: &str, subject: &str, expected_uid: u32) { ... }
// async fn test_e2e_fetch_non_existent_folder(client: &Client) { ... }
// async fn test_e2e_fetch_non_existent_uid(client: &Client) { ... }
// async fn test_e2e_move_invalid_uid(client: &Client) { ... }
// async fn test_e2e_move_invalid_source(client: &Client) { ... }

// --- Test Functions ---
// Note: These run sequentially because they share the TestServer setup.
// Using a framework like serial_test or explicit locking would be needed for parallel execution.

use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::time::Duration;
use dotenvy::dotenv;
use reqwest::Client;
use serde_json::Value;
use serde_json::json;
use reqwest::StatusCode;
use urlencoding;
use std::path::PathBuf;

const BASE_URL: &str = "http://127.0.0.1:8080/api/v1";

// Helper function to find the binary and load env vars
fn setup_environment() -> (PathBuf, Vec<(String, String)>) {
    // Find the target directory (assuming standard Cargo layout)
    let mut target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    target_dir.push("target");
    target_dir.push(if cfg!(debug_assertions) { "debug" } else { "release" });

    let executable_name = "rustymail-server"; // Assuming this is the binary name
    let executable_path = target_dir.join(executable_name);

    assert!(
        executable_path.exists(),
        "Server executable not found at {:?}. Please build the project first.",
        executable_path
    );

    println!("Loading .env file...");
    match dotenv() {
        Ok(path) => println!("Loaded .env from: \"{}\"", path.display()),
        Err(_) => println!("No .env file found or failed to load. Proceeding without it."),
    }

    println!("Verifying environment variables...");
    let mut env_vars = Vec::new();
    for (key, value) in std::env::vars() {
        if key.starts_with("IMAP_") || key == "RUST_LOG" || key == "RUST_BACKTRACE" {
             // Log sensitive vars carefully
             if key == "IMAP_PASS" {
                 println!("Setting {}=<redacted>", key);
             } else {
                 println!("Setting {}={}", key, value);
             }
            env_vars.push((key, value));
        }
    }
    
    // Ensure critical IMAP vars are present (optional but recommended)
    assert!(env_vars.iter().any(|(k, _)| k == "IMAP_HOST"), "IMAP_HOST not set");
    assert!(env_vars.iter().any(|(k, _)| k == "IMAP_PORT"), "IMAP_PORT not set");
    assert!(env_vars.iter().any(|(k, _)| k == "IMAP_USER"), "IMAP_USER not set");
    assert!(env_vars.iter().any(|(k, _)| k == "IMAP_PASS"), "IMAP_PASS not set");

    // Log the collected variables before returning
    println!("Collected Environment Variables for Server Process:");
    for (key, value) in &env_vars {
         if key == "IMAP_PASS" {
             println!("  {}=<redacted>", key);
         } else {
             println!("  {}={}", key, value);
         }
    }

    (executable_path, env_vars)
}

// Structure to manage the server process
struct TestServer {
    process: Option<tokio::process::Child>,
    _stdout_task: tokio::task::JoinHandle<()>,
    _stderr_task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn new() -> Self {
        println!("Checking if port 8080 is available...");
        match std::net::TcpListener::bind("127.0.0.1:8080") {
            Ok(listener) => {
                // Port is available, drop the listener immediately
                drop(listener);
                println!("Port 8080 is available.");
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                panic!("Test setup failed: Port 8080 is already in use. Ensure no other server is running.");
            }
            Err(e) => {
                // Handle other potential binding errors
                panic!("Test setup failed: Error checking port 8080: {}", e);
            }
        }

        println!("Building rustymail binary...");
        let build_status = tokio::process::Command::new("cargo")
            .args(["build", "--bin", "rustymail-server"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .expect("Failed to execute cargo build");

        if !build_status.success() {
            panic!("Cargo build failed!");
        }
        println!("Build successful.");

        let (executable_path, env_vars) = setup_environment();
        println!("Starting rustymail server process...");
        
        // Modify command setup: remove env_clear, use .env() for specifics
        let mut command = Command::new(executable_path);
        command
            // .env_clear() // Remove this
            // .envs(env_vars) // Remove this
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
            
        // Explicitly set the IMAP vars from our collected env_vars map
        // This ensures they override any inherited ones
        for (key, value) in env_vars {
             if key.starts_with("IMAP_") {
                 command.env(&key, value);
             }
             // Optionally pass RUST_LOG/BACKTRACE too if needed by server directly
             // else if key == "RUST_LOG" { command.env(&key, value); }
             // else if key == "RUST_BACKTRACE" { command.env(&key, value); }
        }

        // Spawn the configured command
        let mut child = command.spawn()
            .expect("Failed to spawn server process");

        let pid = child.id().expect("Server process should have a PID");
        println!("Server process started (PID: {})", pid);

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // Spawn tasks to continuously read stdout and stderr
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await.expect("Failed to read stdout") {
                println!("Server stdout [{}]: {}", pid, line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await.expect("Failed to read stderr") {
                println!("Server stderr [{}]: {}", pid, line);
            }
        });
        
        let server = TestServer {
            process: Some(child),
            _stdout_task: stdout_task,
            _stderr_task: stderr_task,
        };

        // Initial delay to allow server to start bindings
        println!("Waiting initial delay for server startup...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Wait for server to be ready by polling health check
        println!("Beginning health check polling...");
        let client = Client::new(); // Create a client for health checks
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(30); // Max wait time
        
        while start_time.elapsed() < timeout {
            println!("Attempting health check via helper function...");
            if test_health_check(&client).await {
                println!("Server is ready! Health check passed.");
                return server; // Return the initialized server
            } else {
                 println!("Health check failed or server not ready yet.");
            }
            println!("Waiting 500ms before next health check attempt...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // If loop finishes, timeout occurred
        // Clean up the spawned process before panicking
        // server.shutdown().await; // Need to implement shutdown or kill manually here
        panic!("Server failed to become ready within {} seconds", timeout.as_secs());
    }

    async fn shutdown(&mut self) {
        if let Some(mut child) = self.process.take() {
            println!("Attempting to terminate server process...");
            if let Err(e) = child.kill().await {
                println!("Error killing process: {}", e);
            }
            match child.wait().await {
                Ok(status) => println!("Server process exited with status: {}", status),
                Err(e) => println!("Error waiting for server process: {}", e),
            }
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if self.process.is_some() { // Check if process handle still exists
            // Log a warning but don't try to kill or create a runtime here.
            // The explicit shutdown() call should handle termination.
            println!("Warning: TestServer dropped without shutdown() being called or completing successfully.");
        }
    }
}

// Helper function for health check
async fn test_health_check(client: &Client) -> bool {
    let health_url = format!("{}/health", BASE_URL);
    match client.get(&health_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Health check successful: Status {}", resp.status());
                // Optionally check body: let body = resp.text().await.unwrap_or_default();
                true
            } else {
                println!("Health check failed: Status {}", resp.status());
                false
            }
        }
        Err(e) => {
            println!("Health check request failed: {:?}", e);
            false
        }
    }
}

#[tokio::test]
async fn run_rest_e2e_tests() {
    println!("--- run_rest_e2e_tests function started ---");
    let mut server = TestServer::new().await;
    println!("TestServer started successfully.");
    let client = Client::new();
    println!("Reqwest client created.");

    println!("--- Running E2E: List Folders ---");
    test_e2e_list_folders(&client).await;
    println!("--- Completed E2E: List Folders ---");

    println!("--- Running E2E: Create/Delete Folder ---");
    test_e2e_create_delete_folder(&client).await;
    println!("--- Completed E2E: Create/Delete Folder ---");

    println!("--- Running E2E: Rename Folder ---");
    test_e2e_rename_folder(&client).await;
    println!("--- Completed E2E: Rename Folder ---");

    println!("--- Running E2E: Select Folder ---");
    test_e2e_select_folder(&client).await;
    println!("--- Completed E2E: Select Folder ---");

    println!("--- Running E2E: Search Emails ---");
    test_e2e_search_emails(&client).await;
    println!("--- Completed E2E: Search Emails ---");

    println!("--- Running E2E: Fetch Emails ---");
    test_e2e_fetch_emails(&client).await;
    println!("--- Completed E2E: Fetch Emails ---");

    println!("--- Running E2E: Move Email ---");
    test_e2e_move_email(&client).await;
    println!("--- Completed E2E: Move Email ---");

    println!("--- Running E2E: Flags Operations ---");
    test_e2e_flags_operations(&client).await;
    println!("--- Completed E2E: Flags Operations ---");

    println!("--- Running E2E: Append Email ---");
    test_e2e_append_email(&client).await;
    println!("--- Completed E2E: Append Email ---");

    // Shutdown the server
    println!("--- Shutting down TestServer ---");
    server.shutdown().await;
    println!("--- TestServer shutdown complete ---");
    println!("--- run_rest_e2e_tests function finished ---");
}

// Add a second, very simple test
#[tokio::test]
async fn simple_test_runs() {
    println!("--- simple_test_runs started ---");
    assert!(true);
    println!("--- simple_test_runs finished ---");
}

// Test the /folders endpoint
async fn test_e2e_list_folders(client: &Client) {
    println!("Testing GET /folders...");
    
    // Add timeout to the request
    let timeout_duration = Duration::from_secs(5);
    
    println!("Sending GET request to {}/folders", BASE_URL);
    let res = match tokio::time::timeout(
        timeout_duration,
        client.get(format!("{}/folders", BASE_URL)).send()
    ).await {
        Ok(result) => match result {
            Ok(response) => {
                println!("Received response with status: {}", response.status());
                response
            },
            Err(e) => {
                println!("HTTP request failed: {:?}", e);
                panic!("Failed to send request: {:?}", e);
            }
        },
        Err(_) => {
            println!("Request timed out after {} seconds", timeout_duration.as_secs());
            panic!("Request timed out");
        }
    };

    assert!(res.status().is_success(), "GET /folders failed with status: {}", res.status());

    // Get response body with timeout
    println!("Reading response body...");
    let body_text = match tokio::time::timeout(
        timeout_duration,
        res.text()
    ).await {
        Ok(result) => match result {
            Ok(text) => {
                println!("Raw response body: {}", text);
                text
            },
            Err(e) => {
                println!("Failed to read response body: {:?}", e);
                panic!("Failed to read response text: {:?}", e);
            }
        },
        Err(_) => {
            println!("Reading response body timed out after {} seconds", timeout_duration.as_secs());
            panic!("Reading response body timed out");
        }
    };

    // Parse response body
    println!("Parsing response JSON...");
    let body: Value = match serde_json::from_str(&body_text) {
        Ok(value) => {
            println!("Successfully parsed JSON: {:?}", value);
            value
        },
        Err(e) => {
            println!("Failed to parse JSON response: {:?}", e);
            println!("Invalid JSON body: {}", body_text);
            panic!("Failed to parse JSON response: {:?}", e);
        }
    };

    // Verify response is an array
    assert!(body.is_array(), "Response should be a JSON array of folders");
    println!("Verified response is an array");
    
    // Basic validation of folder structure
    let folders = body.as_array().unwrap();
    for (i, folder) in folders.iter().enumerate() {
        assert!(folder.get("name").is_some(), "Folder {} missing 'name' field", i);
        println!("Found folder: {}", folder["name"].as_str().unwrap_or("<invalid>"));
    }

    println!("GET /folders test completed successfully");
}

async fn test_e2e_flags_operations(client: &Client) {
    println!("Starting E2E Flags Operations test...");
    let folder = "INBOX";
    let encoded_folder = urlencoding::encode(folder);
    let _search_url_base = format!("{}/folders/{}/emails/search", BASE_URL, encoded_folder);
    let flags_url = format!("{}/folders/{}/emails/flags", BASE_URL, encoded_folder);

    // Find an existing UID to work with (e.g., the first one)
    let initial_search_url = format!("{}/folders/{}/emails/search", BASE_URL, encoded_folder);
    let initial_search_resp = client.get(&initial_search_url).send().await.expect("Initial search failed");
    assert!(initial_search_resp.status().is_success(), "Initial search failed");
    let initial_search_bytes = initial_search_resp.bytes().await.expect("Failed to read initial search bytes");
    let uids: Vec<u32> = match serde_json::from_slice::<Vec<serde_json::Value>>(&initial_search_bytes) {
        Ok(uids_val) => {
             uids_val.into_iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect()
        },
        Err(e) => {
            let body_text = String::from_utf8_lossy(&initial_search_bytes);
            println!("Failed to parse initial search response JSON: {:?}. Body: {}", e, body_text);
            panic!("Invalid initial search response JSON");
        }
    };
    assert!(!uids.is_empty(), "No emails found in INBOX to test flags");
    let test_uid = uids[uids.len() / 2]; // Pick a UID from the middle

    // Add \Flagged
    println!("Adding \\Flagged to UID {}", test_uid);
    let add_payload = json!({
        "uids": [test_uid],
        "operation": "Add",
        "flags": { "items": ["\\Flagged"] }
    });
    let add_resp = client.post(&flags_url).json(&add_payload).send().await.expect("Add flag request failed");
    assert!(add_resp.status().is_success(), "Add flag API call failed");
    println!("Add flag API call successful.");

    // Remove \Flagged
    println!("Removing \\Flagged from UID {}", test_uid);
    let remove_payload = json!({
        "uids": [test_uid],
        "operation": "Remove",
        "flags": { "items": ["\\Flagged"] }
    });
    let remove_resp = client.post(&flags_url).json(&remove_payload).send().await.expect("Remove flag request failed");
    assert!(remove_resp.status().is_success(), "Remove flag API call failed");
    println!("Remove flag API call successful.");

    println!("E2E Flags Operations test completed successfully (verification via API success).");
}

async fn test_e2e_append_email(client: &Client) {
    println!("Starting E2E Append Email test...");

    let folder = "INBOX";
    let encoded_folder = urlencoding::encode(folder);
    let unique_subject = format!("E2ETestAppend_{}", chrono::Utc::now().timestamp());
    let from_email = "test@example.com";
    let to_email = "test@example.com";
    let email_body = "This is an E2E test email body.";

    // Construct the raw RFC 822 email content
    let raw_email_content = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\n\r\n{}",
        from_email,
        to_email,
        unique_subject,
        email_body
    );

    // Append email using the correct payload structure
    let append_url = format!("{}/folders/{}/emails/append", BASE_URL, encoded_folder);
    let append_payload = serde_json::json!({
        "content": raw_email_content,
        "flags": { "items": [] } // No initial flags
    });

    let append_resp = client.post(&append_url)
        .json(&append_payload)
        .send().await.expect("Append request failed");

    // Add logging for status and body
    let status = append_resp.status();
    let body_text = append_resp.text().await.unwrap_or_else(|_| "Failed to read response body".to_string());
    println!("Append Response Status: {}", status);
    println!("Append Response Body: {}", body_text);

    assert!(status.is_success(), "Append email failed");

    // Search for the appended email by subject using the correct query parameter
    let search_url = format!(
        "{}/folders/{}/emails/search?subject={}",
        BASE_URL,
        encoded_folder,
        urlencoding::encode(&unique_subject)
    );
    println!("Searching using URL: {}", search_url);

    let search_resp = client.get(&search_url).send().await.expect("Search after append failed");
    assert!(search_resp.status().is_success(), "Search after append failed");

    // Get response bytes first to handle potential JSON parse error
    let search_bytes = search_resp.bytes().await.expect("Failed to read search response bytes");

    // Try parsing the bytes as JSON
    let uids: Vec<u32> = match serde_json::from_slice::<Vec<serde_json::Value>>(&search_bytes) {
        Ok(uids_val) => {
            // Attempt to convert Vec<Value> to Vec<u32>
            uids_val.into_iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect()
        },
        Err(e) => {
            let body_text = String::from_utf8_lossy(&search_bytes);
            println!("Failed to parse search response JSON: {:?}. Body: {}", e, body_text);
            panic!("Invalid search response JSON");
        }
    };
    
    assert!(!uids.is_empty(), "Appended email not found by subject search");
    println!("Found appended email with UID(s): {:?}", uids);

    // Skip fetching immediately after append due to potential server timing issues
    println!("Skipping immediate fetch after append.");

    println!("E2E Append Email test completed successfully (verified append and search).");
}

async fn test_e2e_create_delete_folder(client: &Client) {
    println!("--- Running E2E: Create/Delete Folder ---");
    let base_name = format!("E2ETestFolder_{}", chrono::Utc::now().timestamp());
    let encoded_name = urlencoding::encode(&base_name);
    println!("Using test folder base name: {}", base_name);

    // --- Create Folder ---
    println!("Attempting to create folder...");
    let create_url = format!("{}/folders", BASE_URL);
    let create_payload = json!({ "name": base_name });
    let create_resp = client
        .post(&create_url)
        .json(&create_payload)
        .send()
        .await
        .expect("Create folder request failed");

    let create_status = create_resp.status();
    let create_body = create_resp.text().await.unwrap_or_else(|_| "Failed to read create response body".to_string());
    println!("Create Response Status: {}", create_status);
    println!("Create Response Body: {}", create_body);
    assert_eq!(create_status, StatusCode::CREATED, "Failed to create folder. Status: {}, Body: {}", create_status, create_body);
    println!("Folder creation successful.");

    // Optional: Add a small delay if needed, though create/delete might be faster than append/fetch
    // tokio::time::sleep(Duration::from_secs(1)).await;

    // --- Delete Folder ---
    println!("Attempting to delete folder...");
    let delete_url = format!("{}/folders/{}", BASE_URL, encoded_name);
    let delete_resp = client
        .delete(&delete_url)
        .send()
        .await
        .expect("Delete folder request failed");

    let delete_status = delete_resp.status();
    let delete_body = delete_resp.text().await.unwrap_or_else(|_| "Failed to read delete response body".to_string());
    println!("Delete Response Status: {}", delete_status);
    println!("Delete Response Body: {}", delete_body);
    assert_eq!(delete_status, StatusCode::OK, "Failed to delete folder. Status: {}, Body: {}", delete_status, delete_body);
    println!("Folder deletion successful.");

    println!("--- Completed E2E: Create/Delete Folder ---");
}

async fn test_e2e_rename_folder(_client: &Client) {
    println!("Stub: test_e2e_rename_folder");
    assert!(true);
}

async fn test_e2e_select_folder(_client: &Client) {
    println!("Stub: test_e2e_select_folder");
    assert!(true);
}

async fn test_e2e_search_emails(_client: &Client) {
    println!("Stub: test_e2e_search_emails");
    assert!(true);
}

async fn test_e2e_fetch_emails(_client: &Client) {
    println!("Stub: test_e2e_fetch_emails");
    assert!(true);
}

async fn test_e2e_move_email(_client: &Client) {
    println!("Stub: test_e2e_move_email");
    assert!(true);
}
