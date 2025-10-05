use log::{error, info, debug};
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use tokio::runtime::Runtime;

/// MCP Stdio adapter for Claude Desktop integration
/// Proxies MCP requests to the backend server's API
fn main() {
    // Initialize logging to stderr (not stdout, which is used for MCP communication)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    info!("Starting RustyMail MCP stdio adapter (API proxy mode)...");

    // Get backend API configuration from environment
    let api_base_url = std::env::var("RUSTYMAIL_API_URL")
        .expect("RUSTYMAIL_API_URL environment variable must be set");
    let api_key = std::env::var("RUSTYMAIL_API_KEY")
        .expect("RUSTYMAIL_API_KEY environment variable must be set");

    info!("Backend API: {}", api_base_url);

    // Create runtime for async operations
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    info!("MCP stdio adapter initialized");

    // Set up stdin reader and stdout writer
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = BufReader::new(stdin);

    // Process messages from stdin - use a persistent loop to keep connection alive
    loop {
        // Read line from stdin
        let mut line = String::new();
        debug!("Waiting for next message from stdin...");
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF reached
                info!("stdin closed (EOF), exiting gracefully");
                break;
            }
            Ok(bytes_read) => {
                debug!("Read {} bytes from stdin", bytes_read);
                let input = line.trim();
                if input.is_empty() {
                    debug!("Empty line received, skipping");
                    continue;
                }

                debug!("Received input: {}", input);

                // Parse the JSON-RPC request
                match serde_json::from_str::<Value>(input) {
                    Ok(request) => {
                        // Handle the request asynchronously
                        let response = rt.block_on(async {
                            handle_mcp_request(request, &client, &api_base_url, &api_key).await
                        });

                        // Only send response if it's not null (notifications don't need responses)
                        if !response.is_null() {
                            // Write response to stdout
                            let response_str = serde_json::to_string(&response)
                                .unwrap_or_else(|e| {
                                    error!("Failed to serialize response: {}", e);
                                    json!({
                                        "jsonrpc": "2.0",
                                        "error": {
                                            "code": -32603,
                                            "message": "Internal error: Failed to serialize response"
                                        }
                                    }).to_string()
                                });

                            // Write response followed by newline
                            if let Err(e) = writeln!(stdout, "{}", response_str) {
                                error!("Failed to write response: {}", e);
                                break;
                            }

                            // Flush stdout to ensure the response is sent immediately
                            if let Err(e) = stdout.flush() {
                                error!("Failed to flush stdout: {}", e);
                                break;
                            }

                            debug!("Sent response: {}", response_str);
                        } else {
                            debug!("No response needed for notification");
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse JSON-RPC request: {}", e);

                        // Send error response
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32700,
                                "message": "Parse error",
                                "data": e.to_string()
                            }
                        });

                        if let Err(e) = writeln!(stdout, "{}", error_response) {
                            error!("Failed to write error response: {}", e);
                            break;
                        }

                        if let Err(e) = stdout.flush() {
                            error!("Failed to flush stdout: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                // Error reading from stdin
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    info!("stdin closed, exiting gracefully");
                } else {
                    error!("Error reading from stdin: {}", e);
                }
                break;
            }
        }
    }

    info!("MCP stdio adapter shutting down");
}

async fn handle_mcp_request(
    request: Value,
    client: &reqwest::Client,
    api_base_url: &str,
    api_key: &str,
) -> Value {
    let method = request.get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let params = request.get("params").cloned().unwrap_or(json!({}));
    let request_id = request.get("id").cloned();

    debug!("Processing MCP request: method={}", method);

    match method {
        "initialize" => {
            // Initialize response should NOT include tools array
            // Tools are discovered via tools/list method after initialization
            info!("Responding to initialize request");

            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "rustymail-mcp",
                        "version": "1.0.0"
                    }
                }
            })
        },
        "tools/call" => {
            // Proxy tool calls to backend API
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let tool_params = params.get("arguments").cloned().unwrap_or(json!({}));

            debug!("Proxying tool call '{}' to backend API", tool_name);

            // Call backend API's MCP execute endpoint
            let api_url = format!("{}/mcp/execute", api_base_url);
            let request_body = json!({
                "tool": tool_name,
                "params": tool_params
            });

            match client.post(&api_url)
                .header("X-API-Key", api_key)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(response) => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            // Convert backend response to MCP format
                            json!({
                                "jsonrpc": "2.0",
                                "id": request_id,
                                "result": {
                                    "content": [{
                                        "type": "text",
                                        "text": serde_json::to_string_pretty(&result).unwrap_or_else(|_| "null".to_string())
                                    }],
                                    "isError": false
                                }
                            })
                        }
                        Err(e) => {
                            error!("Failed to parse backend response: {}", e);
                            json!({
                                "jsonrpc": "2.0",
                                "id": request_id,
                                "error": {
                                    "code": -32603,
                                    "message": format!("Backend response error: {}", e)
                                }
                            })
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to call backend API: {}", e);
                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32603,
                            "message": format!("Backend API call failed: {}", e)
                        }
                    })
                }
            }
        },
        "tools/list" => {
            // Get tools from backend API
            let api_url = format!("{}/mcp/tools", api_base_url);

            debug!("Fetching tool list from backend API");

            match client.get(&api_url)
                .header("X-API-Key", api_key)
                .send()
                .await
            {
                Ok(response) => {
                    match response.json::<Value>().await {
                        Ok(tools_data) => {
                            // Backend returns {"tools": [...]}
                            let tools: Vec<Value> = if let Some(tools_obj) = tools_data.get("tools") {
                                if let Some(arr) = tools_obj.as_array() {
                                    arr.iter().map(|tool| {
                                        // Each tool has name, description, and parameters
                                        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                                        let description = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");
                                        let parameters = tool.get("parameters").cloned().unwrap_or(json!({}));

                                        // Convert parameters object to inputSchema
                                        let properties = if let Some(params_obj) = parameters.as_object() {
                                            params_obj.iter().map(|(key, value)| {
                                                (key.clone(), json!({
                                                    "type": "string",
                                                    "description": value.as_str().unwrap_or("")
                                                }))
                                            }).collect()
                                        } else {
                                            serde_json::Map::new()
                                        };

                                        json!({
                                            "name": name,
                                            "description": description,
                                            "inputSchema": {
                                                "type": "object",
                                                "properties": properties,
                                                "required": []
                                            }
                                        })
                                    }).collect()
                                } else {
                                    vec![]
                                }
                            } else {
                                vec![]
                            };

                            json!({
                                "jsonrpc": "2.0",
                                "id": request_id,
                                "result": {
                                    "tools": tools
                                }
                            })
                        }
                        Err(e) => {
                            error!("Failed to parse tools response: {}", e);
                            json!({
                                "jsonrpc": "2.0",
                                "id": request_id,
                                "error": {
                                    "code": -32603,
                                    "message": format!("Failed to get tools: {}", e)
                                }
                            })
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch tools from backend: {}", e);
                    json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32603,
                            "message": format!("Failed to fetch tools: {}", e)
                        }
                    })
                }
            }
        },
        "notifications/initialized" => {
            // Client has initialized, no response needed for notifications
            info!("Client initialized notification received");
            // Return empty object for notifications (they don't need responses)
            json!(null)
        },
        "ping" => {
            // Handle ping for keepalive
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "pong": true
                }
            })
        },
        _ => {
            // Method not found
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            })
        }
    }
}
