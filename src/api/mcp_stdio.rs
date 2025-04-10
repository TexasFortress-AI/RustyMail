//! Handles MCP (Mail Control Protocol) communication over standard input/output.
//! This module provides a simple, line-based JSON-RPC 2.0 transport mechanism
//! suitable for command-line tools or inter-process communication.

// Standard library imports
use std::io::{self, BufRead, Write};
use std::sync::Arc;

// Async runtime
use tokio::{
    // Import necessary IO traits
    io::{AsyncBufReadExt, AsyncWriteExt},
    sync::Mutex as TokioMutex, // Renamed for clarity
    runtime::Runtime, // Needed for spawn_mcp_stdio_server
};

// Serialization
use serde_json::{self, json, Value}; // Import Value

// Sync primitives

// Logging
use tracing::{debug, error, info, warn};

// Use re-exported MCP types and traits
use crate::mcp::{
    McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError,
    ErrorCode, // Re-exported from error_codes
    McpHandler, // Re-exported from handler
};

// IMAP types (Remove if session factory is not used directly here)
// use crate::imap::ImapSessionFactory;

// Remove potentially outdated import
// use crate::mcp_port::create_mcp_tool_registry;

// --- McpStdioServer Logic ---

/// Implements an MCP server that communicates over standard input and standard output.
///
/// It reads JSON-RPC request objects (one per line) from stdin,
/// processes them using the provided `McpHandler`, and writes JSON-RPC response
/// objects (one per line) to stdout.
pub struct McpStdioServer {
    /// The shared MCP request handler responsible for executing the requested methods.
    mcp_handler: Arc<dyn McpHandler>,
    /// The shared state specific to this stdio communication channel.
    port_state: Arc<TokioMutex<McpPortState>>,
}

impl McpStdioServer {
    /// Creates a new `McpStdioServer`.
    ///
    /// # Arguments
    ///
    /// * `mcp_handler` - An `Arc` pointing to the shared `McpHandler` implementation.
    /// * `port_state` - An `Arc<Mutex<McpPortState>>` holding the state for this stdio session.
    pub fn new(mcp_handler: Arc<dyn McpHandler>, port_state: Arc<TokioMutex<McpPortState>>) -> Self {
        Self {
            mcp_handler,
            port_state,
        }
    }

    /// Runs the main loop of the stdio server.
    ///
    /// This function blocks the current thread, continuously reading lines from stdin,
    /// attempting to parse them as `JsonRpcRequest` objects, handling them using the
    /// `mcp_handler`, and writing the resulting `JsonRpcResponse` back to stdout.
    ///
    /// It handles basic JSON parsing errors by sending a JSON-RPC ParseError response.
    /// Other errors during handling are converted to appropriate `JsonRpcError` responses
    /// by the `mcp_handler` or the underlying logic.
    ///
    /// The loop terminates when stdin reaches EOF.
    ///
    /// # Returns
    ///
    /// An `io::Result<()>` indicating success or an I/O error during reading/writing.
    pub async fn run(&self) -> io::Result<()> {
        info!("Starting MCP stdio server loop...");
        // Use Tokio async stdin/stdout for better integration with async handler
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        let mut writer = tokio::io::BufWriter::new(tokio::io::stdout());
        let mut line = String::new();

        loop {
            line.clear();
            // Read line asynchronously
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    info!("MCP stdio stdin reached EOF. Exiting.");
                    break; // EOF
                }
                Ok(_) => {
                    // Process the line
                    debug!("Received MCP stdio line: {}", line.trim());
                    let response_json_value = match serde_json::from_str::<Value>(&line) {
                        Ok(json_req_value) => {
                            // Successfully parsed JSON, pass to handler
                            self.mcp_handler.handle_request(self.port_state.clone(), json_req_value).await
                        }
                        Err(e) => {
                            // Failed to parse JSON, create a ParseError response
                            error!("Failed to parse MCP stdio request: {}. Line: {}", e, line.trim());
                            let error_response = JsonRpcResponse::parse_error();
                            // Serialize the error response to Value for consistent handling
                            serde_json::to_value(error_response)
                                .unwrap_or_else(|serde_err| {
                                    error!("Failed to serialize ParseError response: {}", serde_err);
                                    json!({ // Fallback basic JSON error
                                        "jsonrpc": "2.0",
                                        "error": {"code": ErrorCode::ParseError as i32, "message": "Parse error"},
                                        "id": null
                                    })
                                })
                        }
                    };

                    // Serialize the final response (success or error) back to JSON string
                    match serde_json::to_string(&response_json_value) {
                        Ok(response_line) => {
                            debug!("Sending MCP stdio response: {}", response_line);
                            if let Err(e) = writer.write_all(response_line.as_bytes()).await {
                                error!("Failed to write response to stdout: {}", e);
                                break; // Stop if we can't write
                            }
                            if let Err(e) = writer.write_all(b"\n").await {
                                error!("Failed to write newline to stdout: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            // This should ideally not happen if JsonRpcResponse serialization is correct
                            error!("Failed to serialize final MCP response: {}", e);
                            let internal_error = JsonRpcResponse::error(
                                response_json_value.get("id").cloned(), // Try to get ID from failed value
                                JsonRpcError::internal_error(format!("Failed to serialize response: {}", e))
                            );
                            if let Ok(err_line) = serde_json::to_string(&internal_error) {
                                let _ = writer.write_all(err_line.as_bytes()).await; // Ignore error
                                let _ = writer.write_all(b"\n").await; // Ignore error
                            }
                        }
                    }

                    // Flush the writer to ensure the line is sent immediately
                    if let Err(e) = writer.flush().await {
                        error!("Failed to flush stdout: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break; // Exit on read error
                }
            }
        }

        info!("MCP stdio server loop finished.");
        Ok(())
    }
}

/// Spawns and runs the MCP stdio server in a new Tokio runtime.
///
/// This function is intended as a convenient entry point for starting the stdio server.
/// It creates the necessary handler and state and blocks until the server exits.
///
/// # Arguments
///
/// * `mcp_handler` - The `McpHandler` implementation to use for processing requests.
///
/// # Returns
///
/// An `io::Result<()>` indicating success or failure in running the server runtime.
pub fn spawn_mcp_stdio_server(mcp_handler: Arc<dyn McpHandler>) -> io::Result<()> {
    info!("Spawning MCP stdio server runtime...");
    let port_state = Arc::new(TokioMutex::new(McpPortState::default()));
    let server = McpStdioServer::new(mcp_handler, port_state);

    // Create a runtime to block on the server's execution
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        if let Err(e) = server.run().await {
            error!("MCP stdio server encountered an error: {}", e);
        }
    });
    info!("MCP stdio server runtime finished.");
    Ok(())
}

// --- Unit Tests --- 
// Tests are currently commented out or limited because testing stdin/stdout interaction
// requires more complex setup (e.g., redirecting streams or refactoring `run` 
// to accept generic AsyncRead/AsyncWrite traits).
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use async_trait::async_trait;
    use crate::mcp::types::McpPortState;
    use tokio::sync::Mutex as TokioMutex;
    // mockall needed if MockMcpHandler is used
    use mockall::{automock, predicate::*};

    // Mock the handler for testing
    #[automock]
    #[async_trait]
    impl McpHandler for MockMcpHandler { // Assuming mockall generated MockMcpHandler
        async fn handle_request(&self, state: Arc<TokioMutex<McpPortState>>, json_req: Value) -> Value {
            // Default mock implementation - echo request or return fixed response
            self.handle_request(state, json_req).await
        }
    }

    // Basic test to check server creation
    #[test]
    fn test_stdio_server_creation() {
        let mock_handler = Arc::new(MockMcpHandler::new());
        let port_state = Arc::new(TokioMutex::new(McpPortState::default()));
        let _server = McpStdioServer::new(mock_handler, port_state);
        // Assert something simple, e.g., server creation doesn't panic
        assert!(true);
    }

    // TODO: Add tests using tokio::io::duplex or similar for stdin/stdout mocking
    //       This requires McpStdioServer::run to be refactored to accept generic readers/writers.

    /* Example structure for a future test using mocked IO */
    /*
    #[tokio::test]
    async fn test_stdio_valid_request_response() {
        let (mut client_stdin, server_stdout) = tokio::io::duplex(1024);
        let (server_stdin, mut client_stdout) = tokio::io::duplex(1024);

        let mut mock_handler = MockMcpHandler::new();
        mock_handler.expect_handle_request()
            .times(1)
            .returning(|_state, req| {
                // Simple echo handler for testing
                let id = req.get("id").cloned();
                let resp = JsonRpcResponse::success(id, req);
                async move { serde_json::to_value(resp).unwrap() }.boxed()
            });

        let handler_arc = Arc::new(mock_handler);
        let port_state = Arc::new(TokioMutex::new(McpPortState::default()));
        let server = McpStdioServer::new(handler_arc, port_state);

        // Spawn the server task with the mocked IO
        let server_handle = tokio::spawn(async move {
            // This requires run to be refactored!
            // server.run_with_io(server_stdin, server_stdout).await 
        });

        // Client side: send request
        let request = json!({ "jsonrpc": "2.0", "method": "test", "id": 1 });
        let request_line = serde_json::to_string(&request).unwrap() + "\n";
        client_stdin.write_all(request_line.as_bytes()).await.unwrap();
        client_stdin.flush().await.unwrap();

        // Client side: read response
        let mut response_buf = String::new();
        let mut reader = tokio::io::BufReader::new(client_stdout);
        reader.read_line(&mut response_buf).await.unwrap();

        // Assert response
        let response_val: Value = serde_json::from_str(&response_buf).unwrap();
        assert_eq!(response_val["id"], 1);
        assert!(response_val["result"].is_object());
        assert_eq!(response_val["result"]["method"], "test");

        // Clean up server task (optional)
        // server_handle.abort();
    }
    */
}

// Removed legacy local error codes module - use crate::mcp::error_codes