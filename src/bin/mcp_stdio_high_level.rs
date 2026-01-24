/// MCP stdio proxy (HIGH-LEVEL VARIANT) - A thin JSON-RPC proxy that forwards requests from stdin to the MCP HTTP backend
///
/// This binary acts as a protocol translation layer between line-oriented JSON-RPC-over-stdin/stdout
/// and HTTP-based JSON-RPC calls to the RustyMail MCP backend server.
///
/// This HIGH-LEVEL variant automatically uses ?variant=high-level to expose only high-level AI-powered tools
/// instead of the full set of low-level tools.
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Backend MCP server URL.
    #[arg(long, env = "MCP_BACKEND_URL")]
    backend_url: String,

    /// Request timeout in seconds.
    #[arg(long, env = "MCP_TIMEOUT", default_value = "30")]
    timeout: u64,

    /// API key for authentication with the backend.
    #[arg(long, env = "RUSTYMAIL_API_KEY")]
    api_key: Option<String>,

    /// Path to the configuration file.
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();

    // Append ?variant=high-level to backend URL if not already present
    if !cli.backend_url.contains("variant=") {
        let separator = if cli.backend_url.contains('?') { "&" } else { "?" };
        cli.backend_url = format!("{}{}variant=high-level", cli.backend_url, separator);
    }

    eprintln!("MCP stdio proxy (HIGH-LEVEL) starting...");
    eprintln!("Backend URL: {}", cli.backend_url);
    eprintln!("Timeout: {}s", cli.timeout);
    if let Some(config_path) = &cli.config {
        eprintln!("Config file: {}", config_path);
    }

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cli.timeout))
        .build()
        .expect("Failed to create HTTP client");

    // Set up stdin/stdout
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    // Main loop: read JSON-RPC requests from stdin, forward to backend, write responses to stdout
    loop {
        line.clear();

        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF reached
                eprintln!("EOF received, exiting gracefully");
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Parse JSON-RPC request
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(request) => {
                        // Validate basic JSON-RPC structure
                        if !request.is_object() {
                            let error = create_error_response(
                                request.get("id"),
                                -32600,
                                "Invalid Request: JSON-RPC request must be an object",
                            );
                            write_response(&mut stdout, &error).await;
                            continue;
                        }

                        // Forward request to backend (with API key if configured)
                        let mut request_builder = client.post(&cli.backend_url).json(&request);
                        if let Some(ref api_key) = cli.api_key {
                            request_builder = request_builder.header("X-API-Key", api_key);
                        }
                        match request_builder.send().await {
                            Ok(response) => {
                                let status = response.status();

                                // Handle 204 No Content - don't write anything to stdout
                                // (Notifications per JSON-RPC 2.0 spec should not receive responses)
                                if status.as_u16() == 204 {
                                    eprintln!("Received 204 No Content - notification acknowledged, no response");
                                    continue;
                                }

                                match response.text().await {
                                    Ok(text) => {
                                        // Write backend response directly to stdout
                                        write_response(&mut stdout, &text).await;
                                    }
                                    Err(e) => {
                                        eprintln!("Error reading response body: {}", e);
                                        let error = create_error_response(
                                            request.get("id"),
                                            -32603,
                                            &format!("Internal error reading response: {}", e),
                                        );
                                        write_response(&mut stdout, &error).await;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error forwarding request to backend: {}", e);
                                let error = create_error_response(
                                    request.get("id"),
                                    -32603,
                                    &format!("Internal error: Failed to connect to backend: {}", e),
                                );
                                write_response(&mut stdout, &error).await;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing JSON: {}", e);
                        let error = create_error_response(
                            None,
                            -32700,
                            &format!("Parse error: {}", e),
                        );
                        write_response(&mut stdout, &error).await;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        }
    }
}

/// Create a JSON-RPC error response
fn create_error_response(id: Option<&Value>, code: i32, message: &str) -> String {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(&Value::Null),
        "error": {
            "code": code,
            "message": message
        }
    });
    response.to_string()
}

/// Write a response to stdout with newline
async fn write_response(stdout: &mut tokio::io::Stdout, response: &str) {
    if let Err(e) = stdout.write_all(response.as_bytes()).await {
        eprintln!("Error writing to stdout: {}", e);
        return;
    }
    if let Err(e) = stdout.write_all(b"\n").await {
        eprintln!("Error writing newline to stdout: {}", e);
        return;
    }
    if let Err(e) = stdout.flush().await {
        eprintln!("Error flushing stdout: {}", e);
    }
}
