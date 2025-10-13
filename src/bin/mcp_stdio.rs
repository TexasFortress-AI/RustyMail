/// MCP stdio proxy - A thin JSON-RPC proxy that forwards requests from stdin to the MCP HTTP backend
///
/// This binary acts as a protocol translation layer between line-oriented JSON-RPC-over-stdin/stdout
/// and HTTP-based JSON-RPC calls to the RustyMail MCP backend server.
///
/// Usage:
///   rustymail-mcp-stdio [OPTIONS]
///
/// Options:
///   --backend-url <URL>  Backend MCP server URL (from MCP_BACKEND_URL env var)
///   --timeout <SECONDS>  Request timeout in seconds (default: 30)
///   --help              Show this help message
///
/// Environment variables:
///   MCP_BACKEND_URL     Backend MCP server URL (overrides default)
///   MCP_TIMEOUT         Request timeout in seconds

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut backend_url = std::env::var("MCP_BACKEND_URL")
        .expect("MCP_BACKEND_URL environment variable must be set (e.g., http://localhost:9437/mcp)");
    let mut timeout_secs = std::env::var("MCP_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .expect("MCP_TIMEOUT environment variable must be set (e.g., 30)");

    // Parse command-line arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--backend-url" => {
                if i + 1 < args.len() {
                    backend_url = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --backend-url requires a value");
                    std::process::exit(1);
                }
            }
            "--timeout" => {
                if i + 1 < args.len() {
                    timeout_secs = match args[i + 1].parse::<u64>() {
                        Ok(t) => t,
                        Err(_) => {
                            eprintln!("Error: --timeout must be a number");
                            std::process::exit(1);
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --timeout requires a value");
                    std::process::exit(1);
                }
            }
            "--help" => {
                print_help();
                std::process::exit(0);
            }
            arg => {
                eprintln!("Error: Unknown argument: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    }

    eprintln!("MCP stdio proxy starting...");
    eprintln!("Backend URL: {}", backend_url);
    eprintln!("Timeout: {}s", timeout_secs);

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
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

                        // Forward request to backend
                        match client.post(&backend_url).json(&request).send().await {
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

/// Print help message
fn print_help() {
    println!("MCP stdio proxy - JSON-RPC proxy for RustyMail MCP backend");
    println!();
    println!("Usage:");
    println!("  rustymail-mcp-stdio [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --backend-url <URL>  Backend MCP server URL (from MCP_BACKEND_URL env var)");
    println!("  --timeout <SECONDS>  Request timeout in seconds (default: 30)");
    println!("  --help              Show this help message");
    println!();
    println!("Environment variables:");
    println!("  MCP_BACKEND_URL     Backend MCP server URL (overrides default)");
    println!("  MCP_TIMEOUT         Request timeout in seconds");
    println!();
    println!("Protocol:");
    println!("  Reads line-delimited JSON-RPC requests from stdin");
    println!("  Writes line-delimited JSON-RPC responses to stdout");
    println!("  Logs errors and warnings to stderr");
}
