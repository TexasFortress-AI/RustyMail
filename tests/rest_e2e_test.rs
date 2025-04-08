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

use std::process::{Command, Stdio};
use tokio::process::Command as TokioCommand;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::time::Duration;
use dotenvy::dotenv;
use reqwest::Client;
use serde_json::{Value, json};
use reqwest::StatusCode;
use urlencoding;
use std::path::PathBuf;
use std::collections::HashMap;
use rustymail::imap::types::{MailboxInfo, Email};

const BASE_URL: &str = "http://127.0.0.1:8080/api/v1";

// Helper function to find the binary and load env vars
fn setup_environment() -> (PathBuf, HashMap<String, String>) {
    let mut target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    target_dir.push("target");
    target_dir.push(if cfg!(debug_assertions) { "debug" } else { "release" });
    let executable_name = "rustymail-server";
    let executable_path = target_dir.join(executable_name);
    assert!(executable_path.exists(), "Server executable not found at {:?}. Build first.", executable_path);

    println!("Loading .env file...");
    dotenv().ok();

    println!("Verifying environment variables...");
    let mut env_vars = HashMap::new();
    for (key, value) in std::env::vars() {
        if key.starts_with("IMAP_") || key == "RUST_LOG" || key == "RUST_BACKTRACE" {
            if key == "IMAP_PASS" { println!("Setting {}=<redacted>", key); }
            else { println!("Setting {}={}", key, value); }
            env_vars.insert(key, value);
        }
    }
    assert!(env_vars.contains_key("IMAP_HOST"), "IMAP_HOST not set");
    assert!(env_vars.contains_key("IMAP_PORT"), "IMAP_PORT not set");
    assert!(env_vars.contains_key("IMAP_USER"), "IMAP_USER not set");
    assert!(env_vars.contains_key("IMAP_PASS"), "IMAP_PASS not set");

    println!("Collected Environment Variables for Server Process:");
    for (key, value) in &env_vars {
        if key == "IMAP_PASS" { println!("  {}=<redacted>", key); }
        else { println!("  {}={}", key, value); }
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
        if std::net::TcpListener::bind("127.0.0.1:8080").is_err() {
             println!("Port 8080 is already in use. Attempting to kill existing process...");
             let _ = Command::new("sh")
                 .arg("-c")
                 .arg("lsof -t -i:8080 | xargs -r kill -9")
                 .output();
             tokio::time::sleep(Duration::from_secs(1)).await;
             if std::net::TcpListener::bind("127.0.0.1:8080").is_err() {
                 panic!("Test setup failed: Port 8080 is still in use after attempting kill.");
             }
             println!("Port 8080 cleared.");
        } else {
            println!("Port 8080 is available.");
        }

        println!("Building rustymail binary...");
        let build_status = Command::new("cargo")
            .arg("build")
            .arg("--bin")
            .arg("rustymail-server")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .expect("Failed to execute cargo build");
        assert!(build_status.success(), "Build failed");
        println!("Build successful.");

        let (executable_path, env_vars) = setup_environment();
        println!("Starting rustymail server process...");

        let mut command = TokioCommand::new(executable_path);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            if key.starts_with("IMAP_") {
                command.env(&key, value);
            }
             else if key == "RUST_LOG" { command.env(&key, value); }
             else if key == "RUST_BACKTRACE" { command.env(&key, value); }
        }

        let mut child = command.spawn().expect("Failed to spawn server process");
        let pid = child.id().expect("Server process should have a PID");
        println!("Server process started (PID: {})", pid);

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

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

        println!("Waiting initial delay for server startup...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("Beginning health check polling...");
        let client = Client::new();
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(30);
        while start_time.elapsed() < timeout {
            println!("Attempting health check via helper function...");
            if test_health_check(&client).await {
                println!("Server is ready! Health check passed.");
                return server;
            } else {
                println!("Health check failed or server not ready yet.");
            }
            println!("Waiting 500ms before next health check attempt...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("Server failed to become ready within {} seconds", timeout.as_secs());
    }

    async fn shutdown(&mut self) {
        println!("--- Shutting down TestServer ---");
        if let Some(mut child) = self.process.take() {
            println!("Attempting to terminate server process...");
            match child.kill().await {
                Ok(_) => println!("Server process kill signal sent."),
                Err(e) => println!("Failed to kill server process: {}", e),
            }
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                 Ok(Ok(status)) => println!("Server process exited with status: {}", status),
                 Ok(Err(e)) => println!("Error waiting for server process exit: {}", e),
                 Err(_) => println!("Timeout waiting for server process exit."),
            }
        } else {
            println!("Server process already gone.");
        }
        println!("--- TestServer shutdown complete ---");
    }
}

// Helper function for health check
async fn test_health_check(client: &Client) -> bool {
    let health_url = format!("{}/health", BASE_URL);
    match client.get(&health_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Health check successful: Status {}", resp.status());
                true
            } else {
                println!("Health check failed: Status {}", resp.status());
                false
            }
        }
        Err(e) => {
            if e.is_connect() {
                 println!("Health check connection refused.");
            } else {
                println!("Health check request failed: {:?}", e);
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn simple_test_runs() {
        println!("--- simple_test_runs started ---");
        assert!(true);
        println!("--- simple_test_runs finished ---");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn run_rest_e2e_tests() {
        println!("--- run_rest_e2e_tests function started ---");
        let mut server = TestServer::new().await;
        println!("TestServer started successfully.");
        let client = Client::new();
        println!("Reqwest client created.");

        test_e2e_list_folders(&client).await;
        test_e2e_create_delete_folder(&client).await;
        test_e2e_rename_folder(&client).await;
        test_e2e_select_folder(&client).await;
        test_e2e_search_emails(&client).await;
        test_e2e_fetch_emails(&client).await;
        test_e2e_flags_operations(&client).await;
        test_e2e_append_email(&client).await;
        test_e2e_move_email(&client).await;

        server.shutdown().await;
        println!("--- run_rest_e2e_tests function finished ---");
    }
}

async fn test_e2e_list_folders(client: &Client) {
    println!("--- Running E2E: List Folders ---");
    println!("Testing GET /folders...");
    let url = format!("{}/folders", BASE_URL);
    println!("Sending GET request to {}", url);
    let resp = client.get(&url).send().await.expect(&format!("Request failed: GET {}", url));

    println!("Received response with status: {}", resp.status());
    let body_text = resp.text().await.unwrap_or_else(|_| "Failed to read response body".to_string());
    println!("Raw response body: {}", body_text);

    let json_body: Result<Value, _> = serde_json::from_str(&body_text);
    assert!(json_body.is_ok(), "Response body is not valid JSON: {}", body_text);
    let json_val = json_body.unwrap();

    assert!(json_val.is_array(), "Response JSON is not an array");
    println!("Verified response is an array");

    let folders = json_val.as_array().unwrap();
    let inbox_exists = folders.iter().any(|f| f["name"] == "INBOX");
    assert!(inbox_exists, "INBOX folder not found in list");
    println!("Found folder: INBOX");

    for folder in folders.iter().filter(|f| f["name"] != "INBOX") {
         if let Some(name) = folder.get("name").and_then(|n| n.as_str()) {
              println!("Found folder: {}", name);
         }
    }

    println!("GET /folders test completed successfully");
    println!("--- Completed E2E: List Folders ---");
}

async fn test_e2e_flags_operations(client: &Client) {
    println!("--- Running E2E: Flags Operations ---");
    println!("Starting E2E Flags Operations test...");
    let folder = "INBOX";
    let encoded_folder = urlencoding::encode(folder);
    let _search_url_base = format!("{}/folders/{}/emails/search", BASE_URL, encoded_folder);
    let flags_url = format!("{}/folders/{}/emails/flags", BASE_URL, encoded_folder);

    let initial_search_url = format!("{}/folders/{}/emails/search", BASE_URL, encoded_folder);
    let initial_search_resp = client.get(&initial_search_url).send().await.expect("Initial search failed");
    assert!(initial_search_resp.status().is_success(), "Initial search failed");
    let initial_search_bytes = initial_search_resp.bytes().await.expect("Failed to read initial search bytes");
    let uids: Vec<u32> = match serde_json::from_slice::<Vec<u32>>(&initial_search_bytes) {
        Ok(uids_val) => uids_val,
        Err(e) => {
            let body_text = String::from_utf8_lossy(&initial_search_bytes);
            println!("Failed to parse initial search response JSON: {:?}. Body: {}", e, body_text);
            panic!("Invalid initial search response JSON");
        }
    };
    assert!(!uids.is_empty(), "No emails found in INBOX to test flags");
    let test_uid = uids[uids.len() / 2];

    println!("Adding \\Flagged to UID {}", test_uid);
    let add_payload = json!({ "uids": [test_uid], "operation": "Add", "flags": { "items": ["\\Flagged"] } });
    let add_resp = client.post(&flags_url).json(&add_payload).send().await.expect("Add flag request failed");
    assert!(add_resp.status().is_success(), "Add flag API call failed: {}", add_resp.text().await.unwrap_or_default());
    println!("Add flag API call successful.");

    println!("Removing \\Flagged from UID {}", test_uid);
    let remove_payload = json!({ "uids": [test_uid], "operation": "Remove", "flags": { "items": ["\\Flagged"] } });
    let remove_resp = client.post(&flags_url).json(&remove_payload).send().await.expect("Remove flag request failed");
    assert!(remove_resp.status().is_success(), "Remove flag API call failed: {}", remove_resp.text().await.unwrap_or_default());
    println!("Remove flag API call successful.");

    println!("E2E Flags Operations test completed successfully (verification via API success).");
     println!("--- Completed E2E: Flags Operations ---");
}

async fn test_e2e_append_email(client: &Client) {
     println!("--- Running E2E: Append Email ---");
    println!("Starting E2E Append Email test...");

    let folder = "INBOX";
    let encoded_folder = urlencoding::encode(folder);
    let unique_subject = format!("E2ETestAppend_{}", chrono::Utc::now().timestamp());
    let from_email = "test@example.com";
    let to_email = "test@example.com";
    let email_body = "This is an E2E test email body.";
    let raw_email_content = format!("From: {}\r\nTo: {}\r\nSubject: {}\r\n\r\n{}", from_email, to_email, unique_subject, email_body);
    let append_url = format!("{}/folders/{}/emails/append", BASE_URL, encoded_folder);
    let append_payload = json!({ "content": raw_email_content, "flags": { "items": [] } });

    let append_resp = client.post(&append_url).json(&append_payload).send().await.expect("Append request failed");
    let status = append_resp.status();
    let body_text = append_resp.text().await.unwrap_or_else(|_| "Failed to read response body".to_string());
    println!("Append Response Status: {}", status);
    println!("Append Response Body: {}", body_text);
    assert!(status.is_success(), "Append email failed");

    let search_url = format!("{}/folders/{}/emails/search?subject={}", BASE_URL, encoded_folder, urlencoding::encode(&unique_subject));
    println!("Searching using URL: {}", search_url);
    let search_resp = client.get(&search_url).send().await.expect("Search after append failed");
    assert!(search_resp.status().is_success(), "Search after append failed");

    let search_bytes = search_resp.bytes().await.expect("Failed to read search response bytes");
    let uids: Vec<u32> = match serde_json::from_slice::<Vec<u32>>(&search_bytes) {
        Ok(uids_val) => uids_val,
        Err(e) => {
            let body_text = String::from_utf8_lossy(&search_bytes);
            println!("Failed to parse search response JSON: {:?}. Body: {}", e, body_text);
            panic!("Invalid search response JSON");
        }
    };
    assert!(!uids.is_empty(), "Appended email not found by subject search");
    println!("Found appended email with UID(s): {:?}", uids);

    println!("Skipping immediate fetch after append.");
    println!("E2E Append Email test completed successfully (verified append and search).");
    println!("--- Completed E2E: Append Email ---");
}

async fn test_e2e_create_delete_folder(client: &Client) {
    println!("--- Running E2E: Create/Delete Folder ---");
    let base_name = format!("E2ETestFolder_{}", chrono::Utc::now().timestamp());
    let encoded_name = urlencoding::encode(&base_name);
    println!("Using test folder base name: {}", base_name);

    println!("Attempting to create folder...");
    let create_url = format!("{}/folders", BASE_URL);
    let create_payload = json!({ "name": base_name });
    let create_resp = client.post(&create_url).json(&create_payload).send().await.expect("Create folder request failed");
    let create_status = create_resp.status();
    let create_body = create_resp.text().await.unwrap_or_else(|_| "Failed to read create response body".to_string());
    println!("Create Response Status: {}", create_status);
    println!("Create Response Body: {}", create_body);
    assert_eq!(create_status, StatusCode::CREATED, "Failed to create folder. Status: {}, Body: {}", create_status, create_body);
    println!("Folder creation successful.");

    println!("Attempting to delete folder...");
    let delete_url = format!("{}/folders/{}", BASE_URL, encoded_name);
    let delete_resp = client.delete(&delete_url).send().await.expect("Delete folder request failed");
    let delete_status = delete_resp.status();
    let delete_body = delete_resp.text().await.unwrap_or_else(|_| "Failed to read delete response body".to_string());
    println!("Delete Response Status: {}", delete_status);
    println!("Delete Response Body: {}", delete_body);
    assert_eq!(delete_status, StatusCode::OK, "Failed to delete folder. Status: {}, Body: {}", delete_status, delete_body);
    println!("Folder deletion successful.");

    println!("--- Completed E2E: Create/Delete Folder ---");
}

async fn test_e2e_rename_folder(client: &Client) {
    println!("--- Running E2E: Rename Folder ---");
    let ts = chrono::Utc::now().timestamp();
    let orig_base_name = format!("RenameOrig_{}", ts);
    let dest_base_name = format!("RenameDest_{}", ts);
    let orig_encoded_name = urlencoding::encode(&orig_base_name);
    let dest_encoded_name = urlencoding::encode(&dest_base_name);
    println!("Using original name: {}, destination name: {}", orig_base_name, dest_base_name);

    // 1. Create the original folder
    println!("Creating original folder...");
    let create_url = format!("{}/folders", BASE_URL);
    let create_payload = json!({ "name": orig_base_name });
    let create_resp = client.post(&create_url).json(&create_payload).send().await.expect("Create folder for rename failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED, "Failed to create original folder");
    println!("Original folder created.");

    // 2. Rename the folder
    println!("Renaming folder...");
    let rename_url = format!("{}/folders/{}", BASE_URL, orig_encoded_name);
    let rename_payload = json!({ "to_name": dest_base_name });
    let rename_resp = client.put(&rename_url).json(&rename_payload).send().await.expect("Rename folder request failed");
    let rename_status = rename_resp.status();
    let rename_body = rename_resp.text().await.unwrap_or_default();
    println!("Rename Response Status: {}", rename_status);
    println!("Rename Response Body: {}", rename_body);
    assert_eq!(rename_status, StatusCode::OK, "Rename folder failed. Status: {}, Body: {}", rename_status, rename_body);
    println!("Folder rename successful.");

    // 3. Verify by deleting the *new* folder name (also cleans up)
    println!("Verifying rename by deleting destination folder...");
    let delete_url = format!("{}/folders/{}", BASE_URL, dest_encoded_name);
    let delete_resp = client.delete(&delete_url).send().await.expect("Delete renamed folder request failed");
    let delete_status = delete_resp.status();
    let delete_body = delete_resp.text().await.unwrap_or_default();
    println!("Delete Response Status: {}", delete_status);
    println!("Delete Response Body: {}", delete_body);
    assert_eq!(delete_status, StatusCode::OK, "Failed to delete *renamed* folder. Status: {}, Body: {}", delete_status, delete_body);
    println!("Destination folder deleted successfully (rename verified).");

    println!("--- Completed E2E: Rename Folder ---");
}

async fn test_e2e_select_folder(client: &Client) {
    println!("--- Running E2E: Select Folder ---");
    let folder_name = "INBOX";
    let encoded_folder = urlencoding::encode(folder_name);
    println!("Selecting folder: {}", folder_name);

    let select_url = format!("{}/folders/{}/select", BASE_URL, encoded_folder);
    let select_resp = client.post(&select_url).send().await.expect("Select folder request failed");
    let select_status = select_resp.status();
    let select_body_bytes = select_resp.bytes().await.expect("Failed to read select response body");
    let select_body_text = String::from_utf8_lossy(&select_body_bytes);

    println!("Select Response Status: {}", select_status);
    println!("Select Response Body: {}", select_body_text);
    assert_eq!(select_status, StatusCode::OK, "Select folder failed. Status: {}, Body: {}", select_status, select_body_text);

    let mailbox_info_res = serde_json::from_slice::<MailboxInfo>(&select_body_bytes);
    assert!(mailbox_info_res.is_ok(), "Failed to parse MailboxInfo from select response: {:?}\nBody: {}", mailbox_info_res.err(), select_body_text);
    let mailbox_info = mailbox_info_res.unwrap();
    println!("Parsed MailboxInfo: {:?}", mailbox_info);

    assert!(mailbox_info.exists > 0, "Mailbox 'exists' count should be greater than 0 for INBOX");
    assert!(!mailbox_info.flags.is_empty(), "Mailbox flags should not be empty");
    assert!(mailbox_info.permanent_flags.contains(&"\\*".to_string()) || !mailbox_info.permanent_flags.is_empty(), "Mailbox permanent flags seem invalid");

    println!("Folder selection successful and MailboxInfo parsed.");
    println!("--- Completed E2E: Select Folder ---");
}

async fn test_e2e_search_emails(client: &Client) {
    println!("--- Running E2E: Search Emails ---");
    let folder_name = "INBOX";
    let encoded_folder = urlencoding::encode(folder_name);

    // 1. Search ALL
    println!("Searching ALL emails in {}...", folder_name);
    let search_all_url = format!("{}/folders/{}/emails/search", BASE_URL, encoded_folder);
    let search_all_resp = client.get(&search_all_url).send().await.expect("Search ALL request failed");
    let search_all_status = search_all_resp.status();
    let search_all_body_bytes = search_all_resp.bytes().await.expect("Failed to read search ALL body");
    let search_all_body_text = String::from_utf8_lossy(&search_all_body_bytes);
    println!("Search ALL Status: {}", search_all_status);
    println!("Search ALL Body: {}", search_all_body_text);
    assert_eq!(search_all_status, StatusCode::OK, "Search ALL failed. Status: {}, Body: {}", search_all_status, search_all_body_text);
    let uids_all_res = serde_json::from_slice::<Vec<u32>>(&search_all_body_bytes);
    assert!(uids_all_res.is_ok(), "Failed to parse UIDs from search ALL response: {:?}\nBody: {}", uids_all_res.err(), search_all_body_text);
    let uids_all = uids_all_res.unwrap();
    assert!(!uids_all.is_empty(), "Search ALL returned empty UID list, INBOX might be empty?");
    println!("Search ALL successful, found {} UIDs: {:?}", uids_all.len(), uids_all);

    // 2. Search by a UID known to exist from the previous search
    let test_uid = uids_all[0]; // Pick the first UID from the list
    println!("Searching for known existing UID {} in {}...", test_uid, folder_name);
    let search_uid_url = format!("{}/folders/{}/emails/search?uid={}", BASE_URL, encoded_folder, test_uid);
    let search_uid_resp = client.get(&search_uid_url).send().await.expect("Search UID request failed");
    let search_uid_status = search_uid_resp.status();
    let search_uid_body_bytes = search_uid_resp.bytes().await.expect("Failed to read search UID body");
    let search_uid_body_text = String::from_utf8_lossy(&search_uid_body_bytes);
    println!("Search UID Status: {}", search_uid_status);
    println!("Search UID Body: {}", search_uid_body_text);
    assert_eq!(search_uid_status, StatusCode::OK, "Search UID failed. Status: {}, Body: {}", search_uid_status, search_uid_body_text);
    let uids_specific_res = serde_json::from_slice::<Vec<u32>>(&search_uid_body_bytes);
    assert!(uids_specific_res.is_ok(), "Failed to parse UIDs from search UID response: {:?}\nBody: {}", uids_specific_res.err(), search_uid_body_text);
    let uids_specific = uids_specific_res.unwrap();
    assert!(uids_specific.contains(&test_uid), "Search for UID {} did not return UID {}", test_uid, test_uid);
    println!("Search UID successful, found UID {}.", test_uid);

    // Add more specific searches if needed (e.g., by subject, from, unseen)

    println!("--- Completed E2E: Search Emails ---");
}

async fn test_e2e_fetch_emails(client: &Client) {
    println!("--- Running E2E: Fetch Emails ---");
    let folder_name = "INBOX";
    let encoded_folder = urlencoding::encode(folder_name);
    let uids_to_fetch = "1,2";

    // 1. Fetch with body
    println!("Fetching UIDs {} with body=true...", uids_to_fetch);
    let fetch_body_url = format!("{}/folders/{}/emails/fetch?uids={}&body=true", BASE_URL, encoded_folder, uids_to_fetch);
    let fetch_body_resp = client.get(&fetch_body_url).send().await.expect("Fetch with body request failed");
    let fetch_body_status = fetch_body_resp.status();
    let fetch_body_bytes = fetch_body_resp.bytes().await.expect("Failed to read fetch body bytes");
    let fetch_body_text = String::from_utf8_lossy(&fetch_body_bytes);
    println!("Fetch (body=true) Status: {}", fetch_body_status);
    println!("Fetch (body=true) Body: {}", fetch_body_text);
    assert_eq!(fetch_body_status, StatusCode::OK, "Fetch with body failed. Status: {}, Body: {}", fetch_body_status, fetch_body_text);
    
    let emails_body_res = serde_json::from_slice::<Vec<Email>>(&fetch_body_bytes);
    assert!(emails_body_res.is_ok(), "Failed to parse emails with body response: {:?}\nBody: {}", emails_body_res.err(), fetch_body_text);
    let emails_body = emails_body_res.unwrap();
    assert_eq!(emails_body.len(), 2, "Expected 2 emails, found {}", emails_body.len());
    assert!(emails_body.iter().any(|e| e.uid == 1), "Email with UID 1 not found");
    assert!(emails_body.iter().any(|e| e.uid == 2), "Email with UID 2 not found");
    assert!(emails_body.iter().all(|e| e.body.is_some()), "Expected email bodies to be present");
    println!("Fetch with body successful.");

    // 2. Fetch without body
    println!("Fetching UIDs {} with body=false...", uids_to_fetch);
    let fetch_no_body_url = format!("{}/folders/{}/emails/fetch?uids={}&body=false", BASE_URL, encoded_folder, uids_to_fetch);
    let fetch_no_body_resp = client.get(&fetch_no_body_url).send().await.expect("Fetch without body request failed");
    let fetch_no_body_status = fetch_no_body_resp.status();
    let fetch_no_body_bytes = fetch_no_body_resp.bytes().await.expect("Failed to read fetch no_body bytes");
    let fetch_no_body_text = String::from_utf8_lossy(&fetch_no_body_bytes);
    println!("Fetch (body=false) Status: {}", fetch_no_body_status);
    println!("Fetch (body=false) Body: {}", fetch_no_body_text);
    assert_eq!(fetch_no_body_status, StatusCode::OK, "Fetch without body failed. Status: {}, Body: {}", fetch_no_body_status, fetch_no_body_text);
    
    let emails_no_body_res = serde_json::from_slice::<Vec<Email>>(&fetch_no_body_bytes);
    assert!(emails_no_body_res.is_ok(), "Failed to parse emails without body response: {:?}\nBody: {}", emails_no_body_res.err(), fetch_no_body_text);
    let emails_no_body = emails_no_body_res.unwrap();
    assert_eq!(emails_no_body.len(), 2, "Expected 2 emails, found {}", emails_no_body.len());
    assert!(emails_no_body.iter().any(|e| e.uid == 1), "Email with UID 1 not found");
    assert!(emails_no_body.iter().any(|e| e.uid == 2), "Email with UID 2 not found");
    assert!(emails_no_body.iter().all(|e| e.body.is_none()), "Expected email bodies to be None");
    println!("Fetch without body successful.");

    println!("--- Completed E2E: Fetch Emails ---");
}

async fn test_e2e_move_email(client: &Client) {
    println!("--- Running E2E: Move Email ---");
    let ts = chrono::Utc::now().timestamp();
    let src_base_name = format!("MoveSrc_{}", ts);
    let dest_base_name = format!("MoveDest_{}", ts);
    let src_encoded_name = urlencoding::encode(&src_base_name);
    let dest_encoded_name = urlencoding::encode(&dest_base_name);
    let dest_full_name = format!("INBOX.{}", dest_base_name); // Full path for IMAP move
    println!("Using source: {}, destination: {}", src_base_name, dest_base_name);

    // --- Setup ---
    // 1. Create source folder
    println!("Creating source folder...");
    let create_src_url = format!("{}/folders", BASE_URL);
    let create_src_payload = json!({ "name": src_base_name });
    let create_src_resp = client.post(&create_src_url).json(&create_src_payload).send().await.expect("Create source folder failed");
    assert_eq!(create_src_resp.status(), StatusCode::CREATED, "Failed to create source folder");

    // 2. Create destination folder
    println!("Creating destination folder...");
    let create_dest_url = format!("{}/folders", BASE_URL);
    let create_dest_payload = json!({ "name": dest_base_name });
    let create_dest_resp = client.post(&create_dest_url).json(&create_dest_payload).send().await.expect("Create dest folder failed");
    assert_eq!(create_dest_resp.status(), StatusCode::CREATED, "Failed to create dest folder");

    // 3. Append email to source folder
    println!("Appending email to source folder...");
    let unique_subject = format!("E2ETestMove_{}", ts);
    let raw_email_content = format!("From: move@test.com\r\nTo: move@test.com\r\nSubject: {}\r\n\r\nMove test body.", unique_subject);
    let append_url = format!("{}/folders/{}/emails/append", BASE_URL, src_encoded_name);
    let append_payload = json!({ "content": raw_email_content, "flags": { "items": [] } });
    let append_resp = client.post(&append_url).json(&append_payload).send().await.expect("Append to source failed");
    assert!(append_resp.status().is_success(), "Append to source failed");

    // 4. Search source folder for the appended email's UID
    println!("Searching source folder for appended email...");
    tokio::time::sleep(Duration::from_secs(2)).await; // Allow time for append/index
    let search_url = format!("{}/folders/{}/emails/search?subject={}", BASE_URL, src_encoded_name, urlencoding::encode(&unique_subject));
    let search_resp = client.get(&search_url).send().await.expect("Search source folder failed");
    assert!(search_resp.status().is_success(), "Search source folder failed");
    let search_bytes = search_resp.bytes().await.expect("Failed to read search source body");
    let uids_res = serde_json::from_slice::<Vec<u32>>(&search_bytes);
    assert!(uids_res.is_ok(), "Failed to parse UIDs from source search: {:?}", uids_res.err());
    let uids = uids_res.unwrap();
    assert_eq!(uids.len(), 1, "Expected 1 UID from source search, found {}", uids.len());
    let email_uid = uids[0];
    println!("Found email with UID {} in source folder.", email_uid);

    // --- Execute Move ---
    // 5. Select the source folder (move works on selected folder)
    println!("Selecting source folder...");
    let select_src_url = format!("{}/folders/{}/select", BASE_URL, src_encoded_name);
    let select_src_resp = client.post(&select_src_url).send().await.expect("Select source folder failed");
    assert_eq!(select_src_resp.status(), StatusCode::OK, "Select source folder failed");

    // 6. Perform the move using the global endpoint
    println!("Moving email UID {} to {}...", email_uid, dest_full_name);
    let move_url = format!("{}/emails/move", BASE_URL);
    let move_payload = json!({ "uids": [email_uid], "destination_folder": dest_full_name }); // Use full path
    let move_resp = client.post(&move_url).json(&move_payload).send().await.expect("Move email request failed");
    let move_status = move_resp.status();
    let move_body = move_resp.text().await.unwrap_or_default();
    println!("Move Response Status: {}", move_status);
    println!("Move Response Body: {}", move_body);
    assert_eq!(move_status, StatusCode::OK, "Move email failed. Status: {}, Body: {}", move_status, move_body);
    println!("Move API call successful.");

    // --- Verification ---
    tokio::time::sleep(Duration::from_secs(2)).await; // Allow time for move/index
    // 7. Search source folder again (should be empty for that UID)
    println!("Verifying email is gone from source folder...");
    let search_src_again_url = format!("{}/folders/{}/emails/search?uid={}", BASE_URL, src_encoded_name, email_uid);
    let search_src_again_resp = client.get(&search_src_again_url).send().await.expect("Search source again failed");
    assert_eq!(search_src_again_resp.status(), StatusCode::OK, "Search source again failed");
     let search_src_again_bytes = search_src_again_resp.bytes().await.expect("Failed read search source again");
     let uids_src_again : Vec<u32> = serde_json::from_slice(&search_src_again_bytes).expect("Parse UIDs source again");
     assert!(uids_src_again.is_empty(), "Email UID {} still found in source folder after move", email_uid);
     println!("Email successfully removed from source.");

    // 8. Select destination folder
    println!("Selecting destination folder...");
    let select_dest_url = format!("{}/folders/{}/select", BASE_URL, dest_encoded_name);
    let select_dest_resp = client.post(&select_dest_url).send().await.expect("Select dest folder failed");
    assert_eq!(select_dest_resp.status(), StatusCode::OK, "Select dest folder failed");

    // 9. Search destination folder (should contain the UID)
    println!("Verifying email exists in destination folder...");
     let search_dest_url = format!("{}/folders/{}/emails/search?uid={}", BASE_URL, dest_encoded_name, email_uid);
    let search_dest_resp = client.get(&search_dest_url).send().await.expect("Search dest folder failed");
    assert_eq!(search_dest_resp.status(), StatusCode::OK, "Search dest folder failed");
     let search_dest_bytes = search_dest_resp.bytes().await.expect("Failed read search dest");
     let uids_dest : Vec<u32> = serde_json::from_slice(&search_dest_bytes).expect("Parse UIDs dest");
     assert!(uids_dest.contains(&email_uid), "Email UID {} not found in destination folder after move", email_uid);
     println!("Email successfully found in destination.");

    // --- Cleanup ---
    println!("Cleaning up move test folders...");
    // Delete source
    let delete_src_url = format!("{}/folders/{}", BASE_URL, src_encoded_name);
    let del_src_resp = client.delete(&delete_src_url).send().await.expect("Delete source folder failed");
    assert_eq!(del_src_resp.status(), StatusCode::OK, "Cleanup: delete source folder failed");
    // Delete destination
    let delete_dest_url = format!("{}/folders/{}", BASE_URL, dest_encoded_name);
    let del_dest_resp = client.delete(&delete_dest_url).send().await.expect("Delete dest folder failed");
     assert_eq!(del_dest_resp.status(), StatusCode::OK, "Cleanup: delete dest folder failed");
    println!("Move test cleanup successful.");

    println!("--- Completed E2E: Move Email ---");
}
