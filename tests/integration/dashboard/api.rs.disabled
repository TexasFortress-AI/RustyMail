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
        let base_url = server_arc.base_url();

        // --- Make some connections to populate client list --- 
        // Create multiple SSE connections
        let mut sse_handles = Vec::new();
        let sse_client = reqwest::Client::new();
        for i in 0..5 { // Create 5 SSE clients
            let sse_test_url = format!("{}/api/dashboard/events", base_url);
            let sse_response_future = sse_client.get(&sse_test_url).send();
            let _sse_response = tokio::time::timeout(Duration::from_secs(5), sse_response_future).await
                .expect("SSE connection timed out")
                .expect(&format!("SSE connection {} failed", i));
            sse_handles.push(_sse_response); // Keep response handle alive
        }
        // Give manager time to register SSE clients
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Make some API requests (these are not typically registered as persistent clients)
        let stats_url = format!("{}/api/dashboard/stats", base_url);
        let api_client = reqwest::Client::new();
        for _ in 0..3 {
            let _stats_res = api_client.get(&stats_url).send().await.expect("Stats request failed");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        // --- End connection setup --- 
        
        let client = reqwest::Client::new();
        let clients_base_url = format!("{}/api/dashboard/clients", base_url);

        // Test default pagination (limit 10)
        println!("Testing default pagination...");
        let response_default = client.get(&clients_base_url)
            .send().await.expect("Default clients request failed");
        assert!(response_default.status().is_success());
        let page_default: PaginatedClients = response_default.json().await.expect("Failed to parse default clients");
        assert!(page_default.pagination.total >= 5, "Expected at least 5 SSE clients, found {}", page_default.pagination.total);
        assert_eq!(page_default.pagination.page, 1);
        assert_eq!(page_default.pagination.limit, 10);
        assert_eq!(page_default.clients.len(), std::cmp::min(page_default.pagination.total, 10), "Client count mismatch on default page");
        let total_clients = page_default.pagination.total;

        // Test custom pagination (limit 2, page 2)
        println!("Testing pagination: limit=2, page=2...");
        let response_page2 = client.get(&clients_base_url)
            .query(&[("limit", "2"), ("page", "2")])
            .send().await.expect("Page 2 clients request failed");
        assert!(response_page2.status().is_success());
        let page2: PaginatedClients = response_page2.json().await.expect("Failed to parse page 2 clients");
        assert_eq!(page2.pagination.total, total_clients);
        assert_eq!(page2.pagination.page, 2);
        assert_eq!(page2.pagination.limit, 2);
        assert!(page2.clients.len() <= 2, "Page 2 should have at most 2 clients");
        if total_clients > 2 { // Only expect clients if total > limit*(page-1)
             assert_eq!(page2.clients.len(), std::cmp::min(2, total_clients.saturating_sub(2)), "Client count mismatch on page 2");
        }

        // Test filtering (assuming SSE clients have 'reqwest' in user agent)
        println!("Testing filtering: filter=reqwest...");
        let response_filtered = client.get(&clients_base_url)
            .query(&[("filter", "reqwest")]) // Filter by expected user agent part
            .send().await.expect("Filtered clients request failed");
        assert!(response_filtered.status().is_success());
        let page_filtered: PaginatedClients = response_filtered.json().await.expect("Failed to parse filtered clients");
        assert!(page_filtered.pagination.total > 0, "Expected filtered clients for 'reqwest'");
        assert!(page_filtered.pagination.total <= total_clients, "Filtered total cannot exceed original total");
        assert!(!page_filtered.clients.is_empty());
        for client_info in &page_filtered.clients {
            assert!(client_info.user_agent.as_ref().map_or(false, |ua| ua.to_lowercase().contains("reqwest")), 
                    "Filtered client {:?} user agent does not contain 'reqwest'", client_info);
        }
        
        // Test filtering with no results
        println!("Testing filtering: filter=NoMatchExpected...");
        let response_no_match = client.get(&clients_base_url)
            .query(&[("filter", "NoMatchExpected")]) 
            .send().await.expect("No match filter request failed");
        assert!(response_no_match.status().is_success());
        let page_no_match: PaginatedClients = response_no_match.json().await.expect("Failed to parse no match filter");
        assert_eq!(page_no_match.pagination.total, 0, "Expected 0 clients for filter 'NoMatchExpected'");
        assert!(page_no_match.clients.is_empty());

        // Cleanup SSE connections
        drop(sse_handles);

        // Shutdown server (handled by Drop trait now)
        // let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        // server_mut.shutdown().await;
        println!("--- Finished test_get_connected_clients ---");
    }

    #[tokio::test]
    #[serial] 
    async fn test_query_chatbot() {
        println!("--- Starting test_query_chatbot ---");
        let server = TestServer::new().await.expect("Failed to start test server");
        let server_arc = Arc::new(server);
        let base_url = server_arc.base_url();

        let client = reqwest::Client::new();
        let chatbot_url = format!("{}/api/dashboard/chatbot/query", base_url);

        // First query: Start a new conversation
        let query1 = ChatbotQuery {
            query: "What is RustyMail?".to_string(),
            conversation_id: None, 
        };

        println!("Sending first chatbot query: {:?}", query1);
        let response1 = client.post(&chatbot_url)
            .json(&query1)
            .timeout(Duration::from_secs(30)) // Increased timeout for potential API call
            .send().await.expect("First chatbot request failed");

        assert!(response1.status().is_success(), "First request failed with status: {}", response1.status());
        let chatbot_response1: ChatbotResponse = response1.json().await
            .expect("Failed to deserialize first chatbot response");
        println!("Received first response: {:?}", chatbot_response1);

        assert!(!chatbot_response1.text.is_empty());
        assert!(!chatbot_response1.conversation_id.is_empty());
        let conversation_id = chatbot_response1.conversation_id.clone();

        // Check if response indicates mock or real AI based on API key presence
        // Check environment variables relevant *at test execution time*
        let using_real_provider = 
            std::env::var("OPENROUTER_API_KEY").is_ok_and(|k| !k.is_empty()) ||
            std::env::var("OPENAI_API_KEY").is_ok_and(|k| !k.is_empty());

        if using_real_provider {
            println!("AI Provider key found (OpenRouter or OpenAI), expecting non-mock response.");
            assert!(!chatbot_response1.text.contains("(Mock Response)"), 
                    "Expected non-mock response with API key, but got: {}", chatbot_response1.text);
        } else {
            println!("No AI Provider key found, expecting mock response.");
            assert!(chatbot_response1.text.contains("(Mock Response)"), 
                    "Expected mock response without API key, but got: {}", chatbot_response1.text);
        }

        // Second query: Continue the conversation
        let query2 = ChatbotQuery {
            query: "Tell me more about its features.".to_string(),
            conversation_id: Some(conversation_id.clone()),
        };

        println!("Sending second chatbot query (continuing conversation {}): {:?}", conversation_id, query2);
        let response2 = client.post(&chatbot_url)
            .json(&query2)
            .timeout(Duration::from_secs(30)) // Increased timeout
            .send().await.expect("Second chatbot request failed");

        assert!(response2.status().is_success(), "Second request failed with status: {}", response2.status());
        let chatbot_response2: ChatbotResponse = response2.json().await
            .expect("Failed to deserialize second chatbot response");
        println!("Received second response: {:?}", chatbot_response2);

        assert!(!chatbot_response2.text.is_empty());
        assert_eq!(chatbot_response2.conversation_id, conversation_id, "Conversation ID mismatch in second response");
        
         // Also check mock/real status for the second response
         if using_real_provider {
            assert!(!chatbot_response2.text.contains("(Mock Response)"), 
                    "Expected non-mock response for second query, but got: {}", chatbot_response2.text);
        } else {
            assert!(chatbot_response2.text.contains("(Mock Response)"), 
                    "Expected mock response for second query, but got: {}", chatbot_response2.text);
        }

        // Shutdown server (handled by Drop trait now)
        // let mut server_mut = Arc::try_unwrap(server_arc).expect("Failed to unwrap Arc for shutdown");
        // server_mut.shutdown().await;
        println!("--- Finished test_query_chatbot ---");
    }
}
