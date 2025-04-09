#[cfg(test)]
mod dashboard_api_tests {
    use std::collections::HashMap;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::sync::Arc;
    use std::time::Duration;

    use dotenvy::dotenv;
    use reqwest;
    use serial_test::serial;
    use tokio::io::AsyncBufReadExt;
    use tokio::process::Command;

    // Assume models are accessible via rustymail crate root
    use rustymail::dashboard::api::models::{
        DashboardStats, 
        ServerConfig,
        PaginatedClients,
        ChatbotQuery,
        ChatbotResponse
    };

    // --- Test Server Infrastructure (Copied from dashboard_sse_test.rs for now) ---
    // Find a free port
    fn find_available_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().expect("Failed to get local address").port()
    }

    // Setup environment and find executable
    fn setup_environment() -> (PathBuf, HashMap<String, String>, u16) {
        let mut target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        target_dir.push("target");
        target_dir.push(if cfg!(debug_assertions) { "debug" } else { "release" });
        let executable_name = "rustymail-server"; // Ensure this matches your binary name
        let executable_path = target_dir.join(executable_name);
        assert!(executable_path.exists(), "Server executable not found at {:?}. Build first.", executable_path);

        dotenv().ok();
        let port = find_available_port();

        let mut env_vars = HashMap::new();
        env_vars.insert("RUST_LOG".to_string(), "debug".to_string());
        env_vars.insert("INTERFACE".to_string(), "rest".to_string());
        env_vars.insert("IMAP_HOST".to_string(), 
            std::env::var("IMAP_HOST").unwrap_or_else(|_| "p3plzcpnl505455.prod.phx3.secureserver.net".to_string()));
        env_vars.insert("IMAP_PORT".to_string(), 
            std::env::var("IMAP_PORT").unwrap_or_else(|_| "993".to_string()));
        env_vars.insert("IMAP_USER".to_string(), 
            std::env::var("IMAP_USER").unwrap_or_else(|_| "info@texasfortress.ai".to_string()));
        env_vars.insert("IMAP_PASS".to_string(), 
            std::env::var("IMAP_PASS").unwrap_or_else(|_| "password".to_string()));
        env_vars.insert("REST_ENABLED".to_string(), "true".to_string()); 
        env_vars.insert("REST_PORT".to_string(), port.to_string());
        env_vars.insert("REST_HOST".to_string(), "127.0.0.1".to_string());
        env_vars.insert("DASHBOARD_ENABLED".to_string(), "true".to_string());
        env_vars.insert("DASHBOARD_PATH".to_string(), "./dashboard-static".to_string()); // Optional

        (executable_path, env_vars, port)
    }

    #[derive(Debug)]
    struct TestServer {
        process: Option<tokio::process::Child>,
        _stdout_task: tokio::task::JoinHandle<()>,
        _stderr_task: tokio::task::JoinHandle<()>,
        port: u16,
        pid_file: Option<String>, // Not currently used
    }

    impl TestServer {
        async fn new() -> io::Result<Self> {
            let (executable_path, env_vars, port) = setup_environment();
            
            println!("Starting server: {:?} on port {}", executable_path, port);
            let mut cmd = Command::new(executable_path);
            cmd.envs(env_vars)
               .stdout(Stdio::piped())
               .stderr(Stdio::piped());

            let mut child = cmd.spawn()?;
            let stdout = child.stdout.take().expect("Failed to get stdout");
            let stderr = child.stderr.take().expect("Failed to get stderr");

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

            let server = TestServer {
                process: Some(child),
                _stdout_task: stdout_handle,
                _stderr_task: stderr_handle,
                port,
                pid_file: None,
            };

            server.wait_for_ready().await;
            Ok(server)
        }

        fn base_url(&self) -> String {
            format!("http://127.0.0.1:{}", self.port)
        }

        async fn wait_for_ready(&self) {
            println!("Waiting for server {} to be ready...", self.base_url());
            let client = reqwest::Client::new();
            let _health_url = format!("{}/health", self.base_url()); // Prefix unused variable
            let base_url = self.base_url();
            let timeout = Duration::from_secs(30);
            let start = std::time::Instant::now();

            while start.elapsed() < timeout {
                 // Try base URL first for general readiness
                match client.get(&base_url).timeout(Duration::from_secs(1)).send().await {
                    Ok(response) if response.status().is_success() => {
                        println!("Server is ready (base URL responded OK)");
                        return;
                    }
                    Ok(_) => { /* Non-success status, maybe still starting */ }
                    Err(_) => { /* Connection refused, definitely not ready */ }
                }
                 // Try health endpoint if it exists
                 // match client.get(&health_url).timeout(Duration::from_secs(1)).send().await { ... }
                 
                 // Fallback: check if port is open
                if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", self.port)).await.is_ok() {
                    println!("Server is ready (port is open)");
                    return;
                }

                println!("Server not ready yet...");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            panic!("Server did not become ready within timeout");
        }

        async fn shutdown(&mut self) {
            println!("Shutting down test server on port {}...", self.port);
            if let Some(mut child) = self.process.take() {
                let pid = child.id();
                println!("Attempting to kill server process with PID {:?}...", pid);
                match child.kill().await {
                    Ok(_) => {
                        println!("Kill signal sent to server process {:?}", pid);
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
            // Abort background tasks regardless of kill success
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
                // Note: Using block_on in a test Drop is generally okay, but avoid in production Drop impls
                // Consider using a dedicated cleanup thread or signal handling for robust production scenarios
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build Tokio runtime for TestServer Drop");
                
                rt.block_on(self.shutdown());
                 println!("TestServer drop: Shutdown finished.");
            }
        }
    }
    // --- End Test Server Infrastructure ---

    #[tokio::test]
    #[serial] 
    async fn test_get_dashboard_stats() {
        println!("--- Starting test_get_dashboard_stats ---");
        let server = TestServer::new().await.expect("Failed to start test server");
        let server_arc = Arc::new(server);
        let base_url = server_arc.base_url(); // Get base URL for convenience

        // --- Simulate some requests to generate metrics --- 
        // Remove unused variable placeholder
        // let metrics_service = { ... };

        let client = reqwest::Client::new();
        let stats_url = format!("{}/api/dashboard/stats", base_url);

        // Make a few requests to populate stats
        for _ in 0..5 {
             let _ = client.get(&stats_url).send().await;
             tokio::time::sleep(Duration::from_millis(50)).await; // Small delay between requests
        }
        tokio::time::sleep(Duration::from_secs(1)).await; // Wait for metrics collection

        println!("Sending final request to {}", stats_url);
        let response = client.get(&stats_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request to /stats endpoint");

        assert!(response.status().is_success(), "Request failed with status: {}", response.status());

        let stats_body = response.text().await.expect("Failed to read response body");
        println!("Received stats body: {}", stats_body);

        let stats: DashboardStats = serde_json::from_str(&stats_body)
            .expect("Failed to deserialize response into DashboardStats");

        // Assertions
        assert!(stats.system_health.cpu_usage >= 0.0);
        assert!(stats.system_health.memory_usage >= 0.0);
        assert!(stats.last_updated.len() > 0);
        // Assert new fields (values might be 0 if middleware isn't calling record methods)
        assert!(stats.requests_per_minute >= 0.0); 
        assert!(stats.average_response_time_ms >= 0.0);

        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        server_mut.shutdown().await;
        println!("--- Finished test_get_dashboard_stats ---");
    }

    #[tokio::test]
    #[serial] 
    async fn test_get_configuration() {
        println!("--- Starting test_get_configuration ---");
        let server = TestServer::new().await.expect("Failed to start test server");
        let server_arc = Arc::new(server);

        let client = reqwest::Client::new();
        let config_url = format!("{}/api/dashboard/config", server_arc.base_url());

        println!("Sending request to {}", config_url);
        let response = client.get(&config_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request to /config endpoint");

        assert!(response.status().is_success(), "Request failed with status: {}", response.status());

        let config_body = response.text().await.expect("Failed to read response body");
        println!("Received config body: {}", config_body);

        // Deserialize and validate the structure
        let config: ServerConfig = serde_json::from_str(&config_body)
            .expect("Failed to deserialize response into ServerConfig");

        // Basic assertions
        assert!(!config.version.is_empty());
        assert!(config.uptime > 0);
        assert!(!config.available_adapters.is_empty());
        assert_eq!(config.active_adapter.id, config.available_adapters[0].id);

        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        server_mut.shutdown().await;
        println!("--- Finished test_get_configuration ---");
    }

    #[tokio::test]
    #[serial] 
    async fn test_get_connected_clients() {
        println!("--- Starting test_get_connected_clients ---");
        let server = TestServer::new().await.expect("Failed to start test server");
        let server_arc = Arc::new(server);

        // --- Make some connections to populate client list --- 
        // 1. SSE Connection (from previous test infrastructure)
        let sse_test_url = format!("{}/api/dashboard/events", server_arc.base_url());
        let sse_client = reqwest::Client::new();
        let sse_response_future = sse_client.get(&sse_test_url).send();
        // We don't need to fully consume the SSE stream, just establish connection
        let _sse_response = tokio::time::timeout(Duration::from_secs(5), sse_response_future).await
            .expect("SSE connection timed out")
            .expect("SSE connection failed");
        // Give manager time to register
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // 2. Basic API Request (acts as another client type conceptually)
        let stats_url = format!("{}/api/dashboard/stats", server_arc.base_url());
        let api_client = reqwest::Client::new();
        let _stats_res = api_client.get(&stats_url).send().await.expect("Stats request failed");
        // Give manager time to register (if it tracks API clients)
        tokio::time::sleep(Duration::from_millis(500)).await;
        // --- End connection setup --- 
        
        let client = reqwest::Client::new();
        let clients_url = format!("{}/api/dashboard/clients", server_arc.base_url());

        // Test default pagination
        println!("Sending request to {} (default page)", clients_url);
        let response = client.get(&clients_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request to /clients endpoint (default)");

        assert!(response.status().is_success(), "Request failed with status: {}", response.status());

        let clients_body = response.text().await.expect("Failed to read response body");
        println!("Received clients body (default): {}", clients_body);

        let clients_page: PaginatedClients = serde_json::from_str(&clients_body)
            .expect("Failed to deserialize response into PaginatedClients");

        assert!(clients_page.pagination.total >= 1, "Expected at least one client (SSE)"); // Should have at least the SSE client
        assert_eq!(clients_page.pagination.page, 1);
        assert_eq!(clients_page.pagination.limit, 10); // Default limit
        assert!(!clients_page.clients.is_empty());
        assert!(clients_page.clients.len() <= 10);

        // TODO: Add tests for pagination parameters (page, limit)
        // TODO: Add tests for filtering (e.g., filter by IP if available)

        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        server_mut.shutdown().await;
        println!("--- Finished test_get_connected_clients ---");
    }

    #[tokio::test]
    #[serial] 
    async fn test_query_chatbot() {
        println!("--- Starting test_query_chatbot ---");
        let server = TestServer::new().await.expect("Failed to start test server");
        let server_arc = Arc::new(server);

        let client = reqwest::Client::new();
        let chatbot_url = format!("{}/api/dashboard/chatbot/query", server_arc.base_url());

        let query = ChatbotQuery {
            query: "hello there".to_string(),
            conversation_id: None, // Start a new conversation
        };

        println!("Sending POST request to {} with query: {:?}", chatbot_url, query);
        let response = client.post(&chatbot_url)
            .json(&query)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("Failed to send request to /chatbot/query endpoint");

        assert!(response.status().is_success(), "Request failed with status: {}", response.status());

        let chatbot_body = response.text().await.expect("Failed to read response body");
        println!("Received chatbot body: {}", chatbot_body);

        let chatbot_response: ChatbotResponse = serde_json::from_str(&chatbot_body)
            .expect("Failed to deserialize response into ChatbotResponse");

        // Basic assertions based on mock response
        assert!(!chatbot_response.text.is_empty());
        assert!(!chatbot_response.conversation_id.is_empty());
        // Check if the response contains expected keywords based on current mock logic
        if query.query.contains("hello") {
            assert!(chatbot_response.text.contains("Hello"));
        } else {
            // Add checks for other mock responses if needed
        }

        // Shutdown server
        let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        server_mut.shutdown().await;
        println!("--- Finished test_query_chatbot ---");
    }
}
