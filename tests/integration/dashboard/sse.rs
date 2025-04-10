// tests/dashboard_sse_test.rs
// Integration tests for the Dashboard SSE implementation
// Run with: cargo test --test dashboard_sse_test --features integration_tests

// Standard library imports
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

// Async runtime and utilities
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_util::bytes::BytesMut;

// Web-related imports
use reqwest;

// Serialization and data types
use serde_json;

// Testing utilities
use dotenvy::dotenv;
use serial_test::serial;

// Shared constants - will be set dynamically
const BASE_PORT: u16 = 8080; // Fallback port if dynamic allocation fails

// Helper struct to track received SSE events
#[derive(Debug, Clone, Default)]
struct SseEvent {
    event_type: String,
    data: String,
}

// Helper function to find the binary and load env vars
fn setup_environment() -> (PathBuf, HashMap<String, String>, u16) {
    let mut target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    target_dir.push("target");
    target_dir.push(if cfg!(debug_assertions) { "debug" } else { "release" });
    let executable_name = "rustymail-server";
    let executable_path = target_dir.join(executable_name);
    assert!(executable_path.exists(), "Server executable not found at {:?}. Build first.", executable_path);

    println!("Loading .env file...");
    dotenv().ok();

    println!("Finding available port...");
    let port = find_available_port();
    println!("Using port {} for test server", port);

    println!("Configuring environment variables...");
    let mut env_vars = HashMap::new();
    
    // Set interface type explicitly
    env_vars.insert("INTERFACE".to_string(), "rest".to_string());
    
    // Configure IMAP settings - using real credentials for integration test
    env_vars.insert("IMAP_HOST".to_string(), 
        std::env::var("IMAP_HOST").unwrap_or_else(|_| "p3plzcpnl505455.prod.phx3.secureserver.net".to_string()));
    env_vars.insert("IMAP_PORT".to_string(), 
        std::env::var("IMAP_PORT").unwrap_or_else(|_| "993".to_string()));
    env_vars.insert("IMAP_USER".to_string(), 
        std::env::var("IMAP_USER").unwrap_or_else(|_| "info@texasfortress.ai".to_string()));
    env_vars.insert("IMAP_PASS".to_string(), 
        std::env::var("IMAP_PASS").unwrap_or_else(|_| "password".to_string())); // Credentials should be in environment
    
    // Configure REST server settings
    env_vars.insert("REST_ENABLED".to_string(), "true".to_string()); 
    env_vars.insert("REST_PORT".to_string(), port.to_string());
    env_vars.insert("REST_HOST".to_string(), "127.0.0.1".to_string());
    
    // Enable dashboard in test mode
    env_vars.insert("DASHBOARD_ENABLED".to_string(), "true".to_string());
    env_vars.insert("DASHBOARD_PATH".to_string(), "./dashboard-static".to_string());
    
    // Add additional test-specific environment variables
    env_vars.insert("RUST_LOG".to_string(), "debug".to_string());
    env_vars.insert("RUST_BACKTRACE".to_string(), "1".to_string());

    // Print what we're using (except password)
    println!("Environment variables configuration:");
    for (key, value) in &env_vars {
        if key == "IMAP_PASS" {
            println!("  {}=<redacted>", key);
        } else {
            println!("  {}={}", key, value);
        }
    }

    (executable_path, env_vars, port)
}

// Find a free port to use for the test server
fn find_available_port() -> u16 {
    for _ in 0..10 {
        // Try to find a random port
        match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                if let Ok(addr) = listener.local_addr() {
                    return addr.port();
                }
            },
            Err(_) => continue,
        }
    }
    
    // If finding a random port fails, use the base port
    BASE_PORT
}

// Structure to manage the server process
#[derive(Debug)]
struct TestServer {
    process: Option<tokio::process::Child>,
    _stdout_task: tokio::task::JoinHandle<()>,
    _stderr_task: tokio::task::JoinHandle<()>,
    port: u16,
    pid_file: Option<String>,
}

impl TestServer {
    async fn new() -> io::Result<Self> {
        // Get executable path, environment variables, and port
        let (executable_path, env_vars, port) = setup_environment();
        
        println!("Starting server: {:?}", executable_path);
        let mut cmd = Command::new(executable_path);
        cmd.envs(env_vars) // Set the environment variables
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let stderr = child.stderr.take().expect("Failed to get stderr");

        // Start background tasks to read stdout and stderr using tokio's async BufReader
        let mut stdout_lines = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_lines = tokio::io::BufReader::new(stderr).lines();

        let stdout_handle = tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                println!("Server stdout: {}", line.trim());
            }
        });

        let stderr_handle = tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_lines.next_line().await {
                eprintln!("Server stderr: {}", line.trim());
            }
        });

        // Create the server struct BEFORE waiting, so we have the port info
        let server = TestServer {
            process: Some(child),
            _stdout_task: stdout_handle,
            _stderr_task: stderr_handle,
            port, // Use the port determined by setup_environment
            pid_file: None, // PID file handling can be added if needed
        };

        // Wait for the server to be ready
        server.wait_for_ready().await;

        Ok(server)
    }

    // Get the base URL for this server instance
    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn wait_for_ready(&self) {
        println!("Waiting for server to be ready...");
        let client = reqwest::Client::new();
        let health_url = format!("http://127.0.0.1:{}/health", self.port);
        let base_url = format!("http://127.0.0.1:{}", self.port);
        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            // First try the health endpoint
            match client.get(&health_url).timeout(Duration::from_secs(1)).send().await {
                Ok(response) if response.status().is_success() => {
                    println!("Server is ready (health check passed)");
                    return;
                }
                _ => {
                    // Then try the base URL
                    match client.get(&base_url).timeout(Duration::from_secs(1)).send().await {
                        Ok(response) => {
                            println!("Server is ready (base URL responded with status: {})", response.status());
                            return;
                        }
                        Err(_) => {
                            // Finally try just connecting to the port
                            match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", self.port)).await {
                                Ok(_) => {
                                    println!("Server is ready (port is open)");
                                    return;
                                }
                                Err(_) => {
                                    // Continue waiting
                                }
                            };
                        }
                    }
                }
            }
            println!("Health check failed or server not ready yet.");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn shutdown(&mut self) {
        println!("Shutting down test server on port {}...", self.port);
        if let Some(mut child) = self.process.take() {
            let pid = child.id();
            println!("Attempting to kill server process with PID {:?}...", pid);
            match child.kill().await {
                Ok(_) => {
                    println!("Kill signal sent to server process {:?}.", pid);
                    // Wait briefly for the process to exit after kill
                    match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                        Ok(Ok(status)) => println!("Server process {:?} exited with status: {}", pid, status),
                        Ok(Err(e)) => eprintln!("Error waiting for server process {:?} exit: {}", pid, e),
                        Err(_) => eprintln!("Timeout waiting for server process {:?} to exit after kill", pid),
                    }
                },
                Err(e) => eprintln!("Error sending kill signal to server process {:?}: {}", pid, e),
            }
        }

        // Abort the background tasks regardless of kill success
        if !self._stdout_task.is_finished() {
             self._stdout_task.abort();
        }
        if !self._stderr_task.is_finished() {
             self._stderr_task.abort();
        }
        println!("Background I/O tasks aborted.");

        // No need for extra sleep here, wait() handled potential delays

        if let Some(path) = &self.pid_file {
            let _ = fs::remove_file(path);
            println!("Removed PID file: {}", path);
        }
        println!("Test server shutdown complete.");
    }
}

// Implement Drop to ensure shutdown is called even on panic
impl Drop for TestServer {
    fn drop(&mut self) {
        // Only attempt shutdown if a process exists
        if self.process.is_some() {
            println!("TestServer drop: Shutting down server process...");
            // Create a new Runtime to run the async shutdown method in a sync context
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime for TestServer Drop");
            
            rt.block_on(self.shutdown());
            println!("TestServer drop: Shutdown finished.");
        }
    }
}

// Helper to parse SSE events from a stream
#[allow(dead_code)] // Keep for potential future use or different test scenarios
async fn parse_sse_events(stdout: tokio::process::ChildStdout) -> io::Result<Vec<SseEvent>> {
    let mut events = Vec::new();
    let mut reader = tokio::io::BufReader::new(stdout);
    let mut line = String::new();
    let mut current_event = SseEvent::default();
    
    while let Ok(n) = reader.read_line(&mut line).await {
        if n == 0 { break; }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_event.data.is_empty() {
                events.push(current_event);
                current_event = SseEvent::default();
            }
        } else if let Some(data) = trimmed.strip_prefix("data: ") {
            current_event.data = data.to_string();
        } else if let Some(event_type) = trimmed.strip_prefix("event: ") {
            current_event.event_type = event_type.to_string();
        }
        line.clear();
    }

    if !current_event.data.is_empty() {
        events.push(current_event);
    }

    Ok(events)
}

// SSE client to connect and receive events
struct SseClient {
    events: Arc<Mutex<Vec<SseEvent>>>,
    stop_signal: Arc<Mutex<bool>>,
    server: Option<Arc<TestServer>>,
}

impl SseClient {
    fn with_server(server: Arc<TestServer>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            stop_signal: Arc::new(Mutex::new(false)),
            server: Some(server),
        }
    }
    
    async fn connect(&mut self) -> tokio::task::JoinHandle<()> {
        let sse_url = format!("{}/api/dashboard/events", self.server.as_ref().expect("Server not set").base_url());
        println!("Connecting to SSE endpoint: {}", sse_url);

        let client = reqwest::Client::new();
        let response = client.get(&sse_url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .expect("Failed to send request to SSE endpoint");

        if !response.status().is_success() {
             let status = response.status();
             let body_text = response.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
             panic!("SSE connection failed with status: {}. Body: {}", status, body_text);
        }
        println!("SSE connection successful (Status: {})", response.status());

        let mut stream = response.bytes_stream();
        let events = self.events.clone();
        let stop_signal = self.stop_signal.clone();

        let handle = tokio::spawn(async move {
            let mut buffer = BytesMut::new();
            loop {
                 tokio::select! {
                     _ = async {
                         loop {
                             if *stop_signal.lock().await { break; }
                             tokio::time::sleep(Duration::from_millis(100)).await;
                         }
                     } => {
                         println!("[SSE Client] Stop signal received, terminating connection.");
                         break;
                     },
                     chunk_result = stream.next() => {
                         match chunk_result {
                             Some(Ok(chunk)) => {
                                 buffer.extend_from_slice(&chunk);
                                 while let Some(event_data) = SseClient::parse_sse_event_from_buffer(&mut buffer) {
                                     println!("[SSE Client] Parsed event: type={}, data_len={}", event_data.event_type, event_data.data.len());
                                     let mut events_guard = events.lock().await;
                                     events_guard.push(event_data);
                                 }
                             },
                             Some(Err(e)) => {
                                 eprintln!("[SSE Client Error] Error reading stream chunk: {}", e);
                                 break;
                             },
                             None => {
                                 println!("[SSE Client] Stream ended.");
                                 break;
                             }
                         }
                     }
                 }
            }
            println!("[SSE Client] Event processing loop finished.");
        });

        handle
    }
    
    async fn stop(&self) {
        let mut stop_signal = self.stop_signal.lock().await;
        *stop_signal = true;
    }
    
    async fn get_events(&self) -> Vec<SseEvent> {
        self.events.lock().await.clone()
    }

    // Helper function to parse a single SSE event from the buffer
    fn parse_sse_event_from_buffer(buffer: &mut BytesMut) -> Option<SseEvent> {
        // Look for a complete event ending with double newline
        let mut end_index = None;
        for i in 0..buffer.len().saturating_sub(1) {
            if buffer[i] == b'\n' && buffer[i + 1] == b'\n' {
                end_index = Some(i + 2);
                break;
            }
        }

        let end_index = end_index?;
        if end_index == 0 { return None; }

        // Extract the event block
        let event_data = buffer.split_to(end_index);
        let mut event_type = String::new();
        let mut data_lines = Vec::new();

        // Convert to string for processing
        if let Ok(event_str) = String::from_utf8(event_data.to_vec()) {
            for line in event_str.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some((field, value)) = line.split_once(':') {
                    let value = value.trim_start();
                    match field {
                        "event" => event_type = value.to_string(),
                        "data" => data_lines.push(value.to_string()),
                        _ => {} // Ignore other fields
                    }
                }
            }
        }

        // If no event type specified, use "message" as default
        if event_type.is_empty() {
            event_type = "message".to_string();
        }

        // Join data lines with newlines
        let data = data_lines.join("\n");

        // Only return event if we have either a non-default event type or non-empty data
        if !data.is_empty() || event_type != "message" {
            Some(SseEvent { event_type, data })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Use the serial attribute to prevent port conflicts between tests
    #[tokio::test]
    #[serial]
    async fn test_sse_connection_receives_welcome() {
        println!("--- Starting Dashboard SSE Connection Test ---");
        let server = TestServer::new().await.expect("Failed to create test server");
        let server_arc = Arc::new(server);
        
        // Create SSE client and connect
        let mut sse_client = SseClient::with_server(Arc::clone(&server_arc));
        let handle = sse_client.connect().await;
        
        // Wait a bit for welcome message
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Get received events
        let events = sse_client.get_events().await;
        
        // Check for welcome event
        let welcome_events: Vec<_> = events.iter()
            .filter(|e| e.event_type == "welcome")
            .collect();
        
        assert!(!welcome_events.is_empty(), "No welcome event received");
        
        // Assert welcome event data
        let welcome_data_str = &welcome_events[0].data;
        println!("Received welcome data: {}", welcome_data_str);
        assert!(welcome_data_str.contains("Connected to RustyMail SSE"), "Welcome data incorrect");
        assert!(welcome_data_str.contains("clientId"), "Welcome data missing clientId");

        // Stop SSE client
        sse_client.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        
        // Explicitly drop the client to release the Arc reference
        drop(sse_client);
        
        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc)
            .expect("Failed to unwrap Arc");
        server_mut.shutdown().await;
    }
    
    #[tokio::test]
    #[serial]
    async fn test_sse_receives_stats_updates() {
        println!("--- Starting Dashboard SSE Stats Updates Test ---");
        let server = TestServer::new().await.expect("Failed to create test server");
        let server_arc = Arc::new(server);
        
        // Create SSE client and connect
        let mut sse_client = SseClient::with_server(Arc::clone(&server_arc));
        let handle = sse_client.connect().await;
        
        // Wait for initial connection and stats events
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Get received events
        let events = sse_client.get_events().await;
        
        // Check for stats_update events
        let stats_events: Vec<_> = events.iter()
            .filter(|e| e.event_type == "stats_update")
            .collect();
        
        assert!(!stats_events.is_empty(), "No stats_update events received within timeout");
        
        // Assert stats event data
        let stats_data_str = &stats_events[0].data;
        println!("Received stats data: {}", stats_data_str);
        let stats_json: Result<serde_json::Value, _> = serde_json::from_str(stats_data_str);
        assert!(stats_json.is_ok(), "Stats data is not valid JSON: {}", stats_json.err().map_or("".to_string(), |e| e.to_string()));
        let stats_value = stats_json.unwrap();
        assert!(stats_value["active_connections"].is_number(), "Stats data missing active_connections");

        // Stop SSE client
        sse_client.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        
        // Explicitly drop the client to release the Arc reference
        drop(sse_client);
        
        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc)
            .expect("Failed to unwrap Arc");
        server_mut.shutdown().await;
    }
    
    #[tokio::test]
    #[serial]
    async fn test_server_starts_and_responds() {
        println!("--- Starting Basic Server Connectivity Test ---");
        let server = TestServer::new().await.expect("Failed to create test server");
        let server_arc = Arc::new(server);
        
        // Simple HTTP request to check if server is up
        let client = reqwest::Client::new();
        let base_url = server_arc.base_url();
        
        println!("Testing connectivity to {}", base_url);
        let response = client.get(&base_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                println!("Server responded with status: {}", resp.status());
            },
            Err(e) => {
                println!("Error connecting to server: {}", e);
                panic!("Failed to connect to server: {}", e);
            }
        }
        
        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc)
            .expect("Failed to unwrap Arc");
        server_mut.shutdown().await;
    }
    
    // We'll keep just two test cases for brevity, but same pattern applies to others
    // Remaining tests would follow the same pattern with dynamic port allocation
} 