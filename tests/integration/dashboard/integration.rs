#[cfg(test)]
mod dashboard_integration_tests {
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

    use rustymail::dashboard::api::models::{
        DashboardStats, 
        ServerConfig,
        PaginatedClients,
        ChatbotQuery,
        ChatbotResponse
    };

    // Helper function to find a free port
    fn find_available_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().expect("Failed to get local address").port()
    }

    // Setup environment and find executable
    fn setup_environment() -> (PathBuf, HashMap<String, String>, u16) {
        let mut target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        target_dir.push("target");
        target_dir.push(if cfg!(debug_assertions) { "debug" } else { "release" });
        let executable_name = "rustymail-server";
        let executable_path = target_dir.join(executable_name);
        assert!(executable_path.exists(), "Server executable not found at {:?}. Build first.", executable_path);

        dotenv().ok();
        let port = find_available_port();

        let mut env_vars = HashMap::new();
        env_vars.insert("RUST_LOG".to_string(), "debug".to_string());
        env_vars.insert("INTERFACE".to_string(), "dashboard".to_string());
        env_vars.insert("IMAP_HOST".to_string(), 
            std::env::var("IMAP_HOST").unwrap_or_else(|_| "localhost".to_string()));
        env_vars.insert("IMAP_PORT".to_string(), 
            std::env::var("IMAP_PORT").unwrap_or_else(|_| "993".to_string()));

        (executable_path, env_vars, port)
    }

    struct TestServer {
        process: Option<tokio::process::Child>,
        _stdout_task: tokio::task::JoinHandle<()>,
        _stderr_task: tokio::task::JoinHandle<()>,
        port: u16,
    }

    impl TestServer {
        async fn new() -> io::Result<Self> {
            let (executable_path, env_vars, port) = setup_environment();
            
            let mut command = Command::new(executable_path);
            command.envs(env_vars);
            command.arg("--port").arg(port.to_string());
            
            let mut child = command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let stdout_task = tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap() > 0 {
                    println!("[Server] {}", line.trim());
                    line.clear();
                }
            });

            let stderr_task = tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stderr);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap() > 0 {
                    eprintln!("[Server Error] {}", line.trim());
                    line.clear();
                }
            });

            Ok(TestServer {
                process: Some(child),
                _stdout_task: stdout_task,
                _stderr_task: stderr_task,
                port,
            })
        }

        fn base_url(&self) -> String {
            format!("http://localhost:{}", self.port)
        }

        async fn wait_for_ready(&self) {
            let client = reqwest::Client::new();
            let url = format!("{}/api/health", self.base_url());
            
            for _ in 0..30 {
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => return,
                    _ => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            }
            panic!("Server did not become ready in time");
        }

        async fn shutdown(&mut self) {
            if let Some(mut child) = self.process.take() {
                child.kill().await.ok();
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(mut child) = self.process.take() {
                let _ = child.kill();
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_dashboard_stats() {
        let mut server = TestServer::new().await.expect("Failed to start server");
        server.wait_for_ready().await;

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/api/dashboard/stats", server.base_url()))
            .send()
            .await
            .expect("Failed to get stats");

        assert!(response.status().is_success());
        let stats: DashboardStats = response.json().await.expect("Failed to parse stats");
        // Check system health fields
        assert!(stats.system_health.memory_usage >= 0.0);
        assert!(stats.system_health.cpu_usage >= 0.0 && stats.system_health.cpu_usage <= 100.0);
        assert!(stats.active_dashboard_sse_clients >= 0);

        server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_dashboard_config() {
        let mut server = TestServer::new().await.expect("Failed to start server");
        server.wait_for_ready().await;

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/api/dashboard/config", server.base_url()))
            .send()
            .await
            .expect("Failed to get config");

        assert!(response.status().is_success());
        let config: ServerConfig = response.json().await.expect("Failed to parse config");
        assert!(!config.active_adapter.name.is_empty());
        assert!(!config.available_adapters.is_empty());
        assert!(config.uptime >= 0);

        server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_dashboard_clients() {
        let mut server = TestServer::new().await.expect("Failed to start server");
        server.wait_for_ready().await;

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/api/dashboard/clients", server.base_url()))
            .send()
            .await
            .expect("Failed to get clients");

        assert!(response.status().is_success());
        let clients: PaginatedClients = response.json().await.expect("Failed to parse clients");
        assert!(clients.pagination.total >= 0);
        assert!(clients.clients.len() <= clients.pagination.total as usize);

        server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_dashboard_chatbot() {
        let mut server = TestServer::new().await.expect("Failed to start server");
        server.wait_for_ready().await;

        let client = reqwest::Client::new();
        let query = ChatbotQuery {
            query: "What is the current server status?".to_string(),
            conversation_id: None,
        };

        let response = client
            .post(&format!("{}/api/dashboard/chatbot", server.base_url()))
            .json(&query)
            .send()
            .await
            .expect("Failed to send chatbot query");

        assert!(response.status().is_success());
        let chatbot_response: ChatbotResponse = response.json().await.expect("Failed to parse chatbot response");
        assert!(!chatbot_response.text.is_empty());

        server.shutdown().await;
    }
}
