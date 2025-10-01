use log::{error, info, debug};
use rustymail::mcp::adapters::sdk::SdkMcpAdapter;
use rustymail::prelude::CloneableImapSessionFactory;
use rustymail::imap::{ImapClient, AsyncImapSessionWrapper, ImapError};
use rustymail::mcp::{McpPortState, McpHandler};
use rustymail::mcp_port;
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use tokio::runtime::Runtime;
use futures_util::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// MCP Stdio adapter for Claude Desktop integration
/// Reads JSON-RPC messages from stdin and writes responses to stdout
fn main() {
    // Initialize logging to stderr (not stdout, which is used for MCP communication)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    info!("Starting RustyMail MCP stdio adapter...");

    // Create runtime for async operations
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Create a placeholder factory that returns an error when called
    // This is fine for the stdio adapter since actual IMAP operations will be handled
    // when real credentials are provided via MCP tools
    let raw_factory: Box<dyn Fn() -> BoxFuture<'static, Result<ImapClient<AsyncImapSessionWrapper>, ImapError>> + Send + Sync> =
        Box::new(|| {
            Box::pin(async move {
                Err(ImapError::Connection("Placeholder factory - IMAP credentials not configured".to_string()))
            })
        });

    let factory = CloneableImapSessionFactory::new(raw_factory);
    let mcp_handler = match SdkMcpAdapter::new(factory) {
        Ok(handler) => handler,
        Err(e) => {
            error!("Failed to initialize MCP handler: {}", e);
            return;
        }
    };

    info!("MCP handler initialized");

    // Set up stdin reader and stdout writer
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin);

    // Process messages from stdin
    for line in reader.lines() {
        match line {
            Ok(input) => {
                if input.trim().is_empty() {
                    continue;
                }

                debug!("Received input: {}", input);

                // Parse the JSON-RPC request
                match serde_json::from_str::<Value>(&input) {
                    Ok(request) => {
                        // Handle the request asynchronously
                        let response = rt.block_on(async {
                            handle_mcp_request(request, &mcp_handler).await
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
                // EOF or error reading from stdin
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

async fn handle_mcp_request(request: Value, mcp_handler: &SdkMcpAdapter) -> Value {
    let method = request.get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let params = request.get("params").cloned().unwrap_or(json!({}));
    let request_id = request.get("id").cloned();

    debug!("Processing MCP request: method={}", method);

    match method {
        "initialize" => {
            // Get the actual tools from the tool registry
            let tool_registry = mcp_port::create_mcp_tool_registry();
            let tools: Vec<_> = tool_registry.keys().map(|name| {
                json!({
                    "name": name,
                    "description": format!("IMAP tool: {}", name),
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                })
            }).collect();

            info!("Responding to initialize with {} tools", tools.len());

            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "serverInfo": {
                        "name": "rustymail-mcp",
                        "version": "1.0.0"
                    },
                    "capabilities": {
                        "tools": true,
                        "resources": false
                    },
                    "tools": tools
                }
            })
        },
        "tools/call" => {
            // Handle tool calls by delegating to the MCP handler
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let tool_params = params.get("arguments").cloned().unwrap_or(json!({}));

            debug!("Calling tool: {} with params: {}", tool_name, tool_params);

            // Create a pseudo request for the handler
            let handler_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": tool_name,
                "params": tool_params
            });

            // Call the actual handler
            let state = Arc::new(TokioMutex::new(McpPortState::default()));
            let response = mcp_handler.handle_request(state, handler_request).await;

            // The response from handle_request is already formatted as JSON-RPC
            response
        },
        "tools/list" => {
            // List available tools from the registry
            let tool_registry = mcp_port::create_mcp_tool_registry();
            let tools: Vec<_> = tool_registry.keys().map(|name| {
                json!({
                    "name": name,
                    "description": format!("IMAP tool: {}", name)
                })
            }).collect();

            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "tools": tools
                }
            })
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