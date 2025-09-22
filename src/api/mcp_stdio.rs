//! Handles MCP (Mail Control Protocol) communication over standard input/output.
//! This module provides a simple, line-based JSON-RPC 2.0 transport mechanism
//! suitable for command-line tools or inter-process communication.

// Standard library imports
use std::{
    collections::HashMap,
    io::{BufReader as StdBufReader, BufWriter as StdBufWriter, Write},
    sync::Arc,
};

// Async runtime
use tokio::{
    select,
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Stdin, Stdout},
    sync::{mpsc, Mutex as TokioMutex},
};
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use futures_util::StreamExt;

// Serialization
use serde_json::{self, json, Value}; // Import Value

// Logging
use tracing::{debug, error, info, warn};

// Crate-local imports
use crate::{ // Group crate imports
    config::Settings,
    mcp::{ // Import necessary MCP types
        handler::McpHandler,
        types::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError},
        ErrorCode,
    },
    session_manager::SessionManager, // Assuming this is used
};

// Main function to run the Stdio service
pub async fn run_stdio_service(
    settings: Settings, 
    mcp_handler: Arc<dyn McpHandler>, 
    session_manager: Arc<SessionManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP Stdio service...");

    let stdin: Stdin = io::stdin();
    let stdout: Stdout = io::stdout();
    let reader = BufReader::new(stdin);
    let mut writer = BufWriter::new(stdout);

    let mut framed_reader = FramedRead::new(reader, LinesCodec::new());

    // Create a shared MCP port state
    let port_state = Arc::new(TokioMutex::new(McpPortState::new(session_manager)));
    
    loop {
        select! {
            line_result = framed_reader.next() => {
                match line_result {
                    Some(Ok(line)) => {
                        debug!("Received line: {}", line);
                        let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);

                        match request {
                            Ok(req) => {
                                // Correct call to handle_mcp_request
                                let response = handle_mcp_request(
                                    mcp_handler.as_ref(), // Pass as &dyn McpHandler
                                    port_state.clone(),
                                    &req // Pass as &JsonRpcRequest
                                ).await;

                                if let Some(resp) = response {
                                    match serde_json::to_string(&resp) {
                                        Ok(resp_str) => {
                                            if let Err(e) = writer.write_all(format!("{}\n", resp_str).as_bytes()).await {
                                                error!("Failed to write response to stdout: {}", e);
                                                break; // Exit on write error
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to serialize response: {}", e);
                                            let err_resp = JsonRpcResponse::error(req.id.clone(), JsonRpcError::internal_error(e.to_string()));
                                            if let Ok(err_str) = serde_json::to_string(&err_resp) {
                                                let _ = writer.write_all(format!("{}\n", err_str).as_bytes()).await;
                                            }
                                        }
                                    }
                                } // Notifications (response is None) are ignored for stdio
                            }
                            Err(e) => {
                                warn!("Failed to parse request as JSON-RPC: {}", e);
                                // ID is unknown on parse error, send None
                                let err_resp = JsonRpcResponse::error(None, JsonRpcError::parse_error());
                                let err_str = serde_json::to_string(&err_resp)
                                    .unwrap_or_else(|e| format!("{{\"jsonrpc\":\"2.0\",\"error\":{{\"code\":-32700,\"message\":\"Parse error: {}\"}},\"id\":null}}", e));
                                
                                let _ = writer.write_all(format!("{}\n", err_str).as_bytes()).await;
                            }
                        }
                        // Flush after every response/error
                        if let Err(e) = writer.flush().await {
                            error!("Failed to flush stdout: {}", e);
                            break;
                        }
                    }
                    Some(Err(LinesCodecError::Io(e))) => {
                        error!("Error reading from stdin: {}", e);
                        break;
                    }
                    Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {
                        error!("Line exceeded maximum length");
                        // ID is likely unknown if line is too long, send None
                        let err_resp = JsonRpcResponse::error(None, JsonRpcError::invalid_request(
                            "Input line exceeded maximum allowed length".to_string()
                        ));
                        if let Ok(err_str) = serde_json::to_string(&err_resp) {
                            if let Err(e) = writer.write_all(format!("{}\n", err_str).as_bytes()).await {
                                error!("Failed to write error response to stdout: {}", e);
                                break;
                            }
                        }
                        let _ = writer.flush().await;
                    }
                    None => {
                        info!("Stdin closed, exiting Stdio service.");
                        break; // End of input stream
                    }
                }
            }
             // TODO: Add mechanism to receive and print MCP Events/Notifications if needed
             // event = event_receiver.recv() => { ... print event ... }
        }
    }

    Ok(())
}

/// Parse and process an MCP request, returning an optional response.
async fn handle_mcp_request(mcp_handler: &dyn McpHandler, port_state: Arc<TokioMutex<McpPortState>>, req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    // Extract ID from the request for the response
    let id = req.id.clone();
    
    // Convert request to JSON Value
    let request_json = match serde_json::to_value(req) {
        Ok(val) => val,
        Err(e) => {
            error!("Failed to convert request to Value: {}", e);
            // Use correct ID here
            return Some(JsonRpcResponse::error(id, JsonRpcError::parse_error()));
        }
    };
    
    // Handle the request with the MCP handler
    let response_val = mcp_handler.handle_request(port_state, request_json).await;
    
    // Construct the JSON-RPC response based on the handler's result
    // Assuming handle_request now returns Result<Value, JsonRpcError>
    match response_val {
        Ok(result_val) => Some(JsonRpcResponse::success(id, result_val)),
        Err(rpc_error) => Some(JsonRpcResponse::error(id, rpc_error)),
    }
}


// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module
    use crate::{ // Import necessary test components
        mcp::{ // Import necessary MCP types for tests
            handler::MockMcpHandler, // Use the mock handler
            // Remove unused Mcp types if JsonRpc is used directly
            // types::{McpMessage, McpRequest, McpResponse, McpNotification, McpParams, McpResult},
        },
        prelude::setup_test_logger, // Assuming setup_test_logger is available in prelude
        session_manager::MockSessionManager, // Import MockSessionManager
    };
    use mockall::{automock, predicate::*};
    use serde_json::json;
    use std::io::Cursor;
    use tokio::io::{DuplexStream, duplex};
    use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

    // Helper function to setup test environment
    fn setup_test_environment() -> (Arc<MockMcpHandler>, DuplexStream, DuplexStream, Arc<TokioMutex<McpPortState>>) {
        setup_test_logger(); // Initialize logger for tests
        let mock_handler = Arc::new(MockMcpHandler::new());
        let (client_stream, service_stream) = duplex(1024); 
        let mock_session_manager = Arc::new(MockSessionManager::new()); // Create mock session manager
        let port_state = Arc::new(TokioMutex::new(McpPortState::new(mock_session_manager))); // Create port state with mock manager
        (mock_handler, client_stream, service_stream, port_state)
    }

    #[tokio::test]
    async fn test_stdio_ping_pong() {
        let (mock_handler, client_stream, mut service_stream, port_state) = setup_test_environment();

        // Configure mock handler expectation
        // handle_request takes port_state, request_value
        let mut handler_clone = Arc::clone(&mock_handler); // Use Arc::clone
        handler_clone.expect_handle_request()
            .withf(move |state, req_val| { // Use move closure
                // Check state if needed: state.lock().await...
                req_val["method"] == "ping" && req_val["id"] == 1
            })
            .times(1)
            .returning(|_state, _req| {
                // Return a successful Pong result value
                Ok(json!("pong")) // Return just the result part
            });

        // Spawn the service task (simplified version reading/writing to the duplex stream)
        let service_task = tokio::spawn(async move {
            let reader = BufReader::new(service_stream);
            let mut writer = BufWriter::new(io::stdout()); // Write to stdout for test visibility for now
            let mut framed_reader = FramedRead::new(reader, LinesCodec::new());
            
            if let Some(Ok(line)) = framed_reader.next().await {
                let req: JsonRpcRequest = serde_json::from_str(&line).unwrap();
                let response = handle_mcp_request(handler_clone.as_ref(), port_state, &req).await;
                if let Some(resp) = response {
                    let resp_str = serde_json::to_string(&resp).unwrap();
                    println!("Service responding: {}", resp_str); // Print for debugging
                    // Assert response structure
                    assert_eq!(resp.jsonrpc, "2.0");
                    assert_eq!(resp.id, Some(json!(1)));
                    assert_eq!(resp.result, Some(json!("pong")));
                    assert!(resp.error.is_none());
                    // In a real test, write back to the writer connected to the client stream
                    // writer.write_all(format!("{}\n", resp_str).as_bytes()).await.unwrap();
                    // writer.flush().await.unwrap();
                }
            }
        });

        // Simulate client sending a ping request
        let mut client_writer = FramedWrite::new(client_stream, LinesCodec::new());
        let ping_request = json!({ "jsonrpc": "2.0", "method": "ping", "id": 1 });
        client_writer.send(serde_json::to_string(&ping_request).unwrap()).await.unwrap();

        // Wait for the service task to complete (or timeout)
        match tokio::time::timeout(Duration::from_secs(1), service_task).await {
            Ok(Ok(_)) => { /* Test finished */ }
            Ok(Err(e)) => panic!("Service task failed: {}", e),
            Err(_) => panic!("Test timed out"),
        }
        
        // TODO: Read response from client stream if writing back
    }
}