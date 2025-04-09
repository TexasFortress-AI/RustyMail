// tests/dashboard_sse_test.rs
// Integration tests for the Dashboard SSE implementation
// Run with: cargo test --test dashboard_sse_test --features integration_tests

use std::sync::Arc;
use std::process::{Command, Stdio};
use tokio::process::Command as TokioCommand;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncReadExt};
use std::time::Duration;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::Mutex;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use dotenvy::dotenv;
use regex::Regex;
use lazy_static::lazy_static;

// Shared constants
const BASE_URL: &str = "http://127.0.0.1:8080";
const SSE_ENDPOINT: &str = "/dashboard/api/events";
const API_STATS_ENDPOINT: &str = "/dashboard/api/stats";

// Helper struct to track received SSE events
#[derive(Debug, Clone)]
struct SseEvent {
    event_type: String,
    data: String,
    id: Option<String>,
}

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
    
    // Enable dashboard in test mode
    env_vars.insert("DASHBOARD_ENABLED".to_string(), "true".to_string());
    
    // Add additional test-specific environment variables if needed
    env_vars.insert("RUST_LOG".to_string(), "debug".to_string());

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
        println!("Starting rustymail server process from {:?}...", executable_path);

        let mut command = TokioCommand::new(executable_path);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            command.env(&key, value);
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
            println!("Attempting health check...");
            if let Ok(resp) = client.get(format!("{}/api/v1/health", BASE_URL)).send().await {
                if resp.status().is_success() {
                    println!("Server is ready! Health check passed.");
                    return server;
                }
            }
            println!("Health check failed or server not ready yet.");
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

// Helper to parse SSE events from a stream
async fn parse_sse_events(bytes: &[u8]) -> Vec<SseEvent> {
    lazy_static! {
        static ref EVENT_REGEX: Regex = Regex::new(r"(?m)^event: ([^\n]+)\ndata: ([^\n]+)(?:\nid: ([^\n]+))?").unwrap();
        static ref COMMENT_REGEX: Regex = Regex::new(r"(?m)^: ([^\n]+)").unwrap();
    }
    
    let content = String::from_utf8_lossy(bytes);
    let mut events = Vec::new();
    
    // Parse normal events (event + data)
    for captures in EVENT_REGEX.captures_iter(&content) {
        let event_type = captures.get(1).map_or("", |m| m.as_str()).to_string();
        let data = captures.get(2).map_or("", |m| m.as_str()).to_string();
        let id = captures.get(3).map(|m| m.as_str().to_string());
        
        events.push(SseEvent {
            event_type,
            data,
            id,
        });
    }
    
    // Parse comment events (heartbeats)
    for captures in COMMENT_REGEX.captures_iter(&content) {
        let comment = captures.get(1).map_or("", |m| m.as_str()).to_string();
        
        events.push(SseEvent {
            event_type: "comment".to_string(),
            data: comment,
            id: None,
        });
    }
    
    events
}

// SSE client to connect and receive events
struct SseClient {
    events: Arc<Mutex<Vec<SseEvent>>>,
    client_id: Option<String>,
    stop_signal: Arc<Mutex<bool>>,
}

impl SseClient {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            client_id: None,
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }
    
    async fn connect(&mut self) -> tokio::task::JoinHandle<()> {
        let events_clone = Arc::clone(&self.events);
        let stop_signal_clone = Arc::clone(&self.stop_signal);
        let client_id_clone = Arc::new(Mutex::new(None::<String>));
        
        tokio::spawn(async move {
            let client = Client::new();
            let mut response = client
                .get(format!("{}{}", BASE_URL, SSE_ENDPOINT))
                .header("Accept", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .send()
                .await
                .expect("Failed to connect to SSE endpoint");
            
            // Process the stream of SSE events
            let mut buffer = Vec::new();
            loop {
                // Check if we should stop
                {
                    let should_stop = *stop_signal_clone.lock().await;
                    if should_stop {
                        println!("SSE client stopping as requested");
                        break;
                    }
                }
                
                // Read some data from the response
                let mut chunk = Vec::new();
                match tokio::time::timeout(Duration::from_secs(1), response.chunk()).await {
                    Ok(Ok(Some(bytes))) => {
                        chunk.extend_from_slice(&bytes);
                        buffer.extend_from_slice(&bytes);
                        
                        // Parse events from the buffer
                        let new_events = parse_sse_events(&buffer).await;
                        
                        // Store the events
                        let mut events = events_clone.lock().await;
                        for event in &new_events {
                            // Extract client ID from welcome event
                            if event.event_type == "welcome" && client_id_clone.lock().await.is_none() {
                                if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                                    if let Some(id) = json.get("clientId").and_then(|id| id.as_str()) {
                                        *client_id_clone.lock().await = Some(id.to_string());
                                        println!("Extracted client ID: {}", id);
                                    }
                                }
                            }
                            events.push(event.clone());
                        }
                        
                        println!("Received {} new SSE events, total: {}", new_events.len(), events.len());
                    },
                    Ok(Ok(None)) => {
                        // End of stream
                        println!("SSE stream ended");
                        break;
                    },
                    Ok(Err(e)) => {
                        eprintln!("Error reading from SSE stream: {:?}", e);
                        break;
                    },
                    Err(_) => {
                        // Timeout, no data received
                        continue;
                    }
                }
            }
        })
    }
    
    async fn stop(&self) {
        let mut signal = self.stop_signal.lock().await;
        *signal = true;
    }
    
    async fn get_events(&self) -> Vec<SseEvent> {
        self.events.lock().await.clone()
    }
    
    async fn get_client_id(&self) -> Option<String> {
        self.client_id.clone()
    }
    
    async fn get_events_by_type(&self, event_type: &str) -> Vec<SseEvent> {
        self.events.lock().await
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sse_connection_receives_welcome() {
        println!("--- Starting Dashboard SSE Connection Test ---");
        let mut server = TestServer::new().await;
        
        // Create SSE client and connect
        let mut sse_client = SseClient::new();
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
        
        // Stop SSE client
        sse_client.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        
        // Shutdown server
        server.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_sse_receives_stats_updates() {
        println!("--- Starting Dashboard SSE Stats Updates Test ---");
        let mut server = TestServer::new().await;
        
        // Create SSE client and connect
        let mut sse_client = SseClient::new();
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
        
        // Stop SSE client
        sse_client.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        
        // Shutdown server
        server.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_multiple_concurrent_sse_clients() {
        println!("--- Starting Multiple SSE Clients Test ---");
        let mut server = TestServer::new().await;
        
        // Create multiple SSE clients
        const CLIENT_COUNT: usize = 3;
        let mut clients = Vec::with_capacity(CLIENT_COUNT);
        let mut handles = Vec::with_capacity(CLIENT_COUNT);
        
        for i in 0..CLIENT_COUNT {
            let mut client = SseClient::new();
            let handle = client.connect().await;
            clients.push(client);
            handles.push(handle);
            println!("Started SSE client {}", i + 1);
        }
        
        // Wait for initial events
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Verify each client received welcome + other events
        for (i, client) in clients.iter().enumerate() {
            let events = client.get_events().await;
            println!("Client {} received {} events", i + 1, events.len());
            
            // Check for welcome event
            let welcome_events: Vec<_> = events.iter()
                .filter(|e| e.event_type == "welcome")
                .collect();
            assert!(!welcome_events.is_empty(), "Client {} didn't receive welcome event", i + 1);
            
            // Check for stats events
            let stats_events: Vec<_> = events.iter()
                .filter(|e| e.event_type == "stats_update")
                .collect();
            assert!(!stats_events.is_empty(), "Client {} didn't receive stats events", i + 1);
        }
        
        // Stop SSE clients
        for client in &clients {
            client.stop().await;
        }
        
        // Wait for all clients to disconnect
        for handle in handles {
            let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        }
        
        // Shutdown server
        server.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_sse_client_connected_events() {
        println!("--- Starting SSE Client Connected Events Test ---");
        let mut server = TestServer::new().await;
        
        // Connect first client
        let mut client1 = SseClient::new();
        let handle1 = client1.connect().await;
        
        // Wait for first client to connect
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Connect second client
        let mut client2 = SseClient::new();
        let handle2 = client2.connect().await;
        
        // Wait for second client to connect and events to propagate
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // First client should receive client_connected event for second client
        let connected_events = client1.get_events_by_type("client_connected").await;
        assert!(connected_events.len() >= 1, "First client didn't receive client_connected events");
        
        // Stop clients
        client1.stop().await;
        client2.stop().await;
        
        let _ = tokio::time::timeout(Duration::from_secs(3), handle1).await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle2).await;
        
        // Shutdown server
        server.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_sse_heartbeat() {
        println!("--- Starting SSE Heartbeat Test ---");
        let mut server = TestServer::new().await;
        
        // Create SSE client and connect
        let mut sse_client = SseClient::new();
        let handle = sse_client.connect().await;
        
        // Wait long enough to receive heartbeats (they should come every 15 seconds)
        tokio::time::sleep(Duration::from_secs(20)).await;
        
        // Get received events
        let events = sse_client.get_events().await;
        
        // Check for heartbeat comments
        let heartbeat_events: Vec<_> = events.iter()
            .filter(|e| e.event_type == "comment" && e.data == "heartbeat")
            .collect();
        
        assert!(!heartbeat_events.is_empty(), "No heartbeat events received within timeout");
        
        // Stop SSE client
        sse_client.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        
        // Shutdown server
        server.shutdown().await;
    }
} 