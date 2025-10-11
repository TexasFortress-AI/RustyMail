//! Integration tests for rustymail-mcp-stdio binary
//! Tests the JSON-RPC stdio proxy that forwards requests to the backend

use serde_json::json;
use std::process::{Command, Stdio};
use std::io::Write;
use serial_test::serial;
use std::sync::Once;

static INIT: Once = Once::new();

/// Helper to set up test environment and ensure binary is built
fn setup_test_env() {
    INIT.call_once(|| {
        // Build the binary once before all tests
        let status = Command::new("cargo")
            .args(&["build", "--bin", "rustymail-mcp-stdio"])
            .status()
            .expect("Failed to build rustymail-mcp-stdio binary");

        assert!(status.success(), "Failed to build rustymail-mcp-stdio binary");
    });

    std::env::set_var("MCP_BACKEND_URL", "http://localhost:9437/mcp");
    std::env::set_var("MCP_TIMEOUT", "30");
}

/// Get the path to the built binary
fn get_binary_path() -> std::path::PathBuf {
    let mut path = std::env::current_dir().expect("Failed to get current dir");
    path.push("target");
    path.push("debug");
    path.push("rustymail-mcp-stdio");
    path
}

#[test]
#[serial]
fn test_stdio_proxy_help_flag() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio --help ===");

    let output = Command::new(get_binary_path())
        .arg("--help")
        .output()
        .expect("Failed to execute binary");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Binary should exit successfully with --help");
    assert!(stdout.contains("MCP stdio proxy"), "Help text should contain description");
    assert!(stdout.contains("--backend-url"), "Help text should document --backend-url option");
    assert!(stdout.contains("--timeout"), "Help text should document --timeout option");
    assert!(stdout.contains("MCP_BACKEND_URL"), "Help text should document MCP_BACKEND_URL env var");

    println!("✓ --help flag displays usage information");
}

#[test]
#[serial]
fn test_stdio_proxy_missing_env_vars() {
    setup_test_env();  // Build the binary first
    println!("=== Testing rustymail-mcp-stdio without required env vars ===");

    // Unset required environment variables
    std::env::remove_var("MCP_BACKEND_URL");
    std::env::remove_var("MCP_TIMEOUT");

    let output = Command::new(get_binary_path())
        .output()
        .expect("Failed to execute binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Binary should fail without required env vars");
    assert!(
        stderr.contains("MCP_BACKEND_URL") || stderr.contains("must be set"),
        "Error should mention missing environment variable"
    );

    println!("✓ Binary fails appropriately when env vars are missing");

    // Restore for other tests
    setup_test_env();
}

#[test]
#[serial]
fn test_stdio_proxy_invalid_timeout_arg() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with invalid --timeout ===");

    let output = Command::new(get_binary_path())
        .args(&["--timeout", "invalid"])
        .output()
        .expect("Failed to execute binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Binary should fail with invalid timeout");
    assert!(
        stderr.contains("must be a number") || stderr.contains("timeout"),
        "Error should mention invalid timeout: {}",
        stderr
    );

    println!("✓ Binary rejects invalid timeout values");
}

#[test]
#[serial]
fn test_stdio_proxy_unknown_argument() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with unknown argument ===");

    let output = Command::new(get_binary_path())
        .arg("--unknown-flag")
        .output()
        .expect("Failed to execute binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "Binary should fail with unknown argument");
    assert!(
        stderr.contains("Unknown argument") || stderr.contains("unknown-flag"),
        "Error should mention unknown argument: {}",
        stderr
    );

    println!("✓ Binary rejects unknown arguments");
}

#[test]
#[serial]
#[ignore] // Requires running backend server
fn test_stdio_proxy_valid_json_rpc_request() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with valid JSON-RPC request ===");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Send a valid initialize request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "{}", request.to_string()).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Give it time to process
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Terminate the child process
    child.kill().expect("Failed to kill child process");
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Stdout: {}", stdout);
    println!("Stderr: {}", stderr);

    // Verify response structure (if backend is running)
    if stdout.contains("jsonrpc") {
        assert!(stdout.contains("\"jsonrpc\":\"2.0\""), "Response should be valid JSON-RPC");
        assert!(stdout.contains("\"id\":1"), "Response should include request ID");
        println!("✓ Valid JSON-RPC request forwarded successfully");
    } else {
        println!("⚠ Backend not running - skipping response validation");
    }
}

#[test]
#[serial]
fn test_stdio_proxy_malformed_json() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with malformed JSON ===");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Send malformed JSON
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "{{invalid json}}").expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Give it time to process and flush output (increased from 100ms to 500ms)
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Close stdin to signal end of input
    drop(child.stdin.take());

    // Wait for process to exit gracefully
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Stdout: {}", stdout);
    println!("Stderr: {}", stderr);

    // Should return JSON-RPC parse error
    assert!(stdout.contains("-32700") || stdout.contains("Parse error"),
        "Should return JSON-RPC parse error (-32700). Stdout: {}", stdout);
    assert!(stderr.contains("Error parsing JSON"),
        "Should log parse error to stderr. Stderr: {}", stderr);

    println!("✓ Malformed JSON handled correctly with error response");
}

#[test]
#[serial]
fn test_stdio_proxy_invalid_jsonrpc_structure() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with invalid JSON-RPC structure ===");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Send valid JSON but not a JSON-RPC object (array instead)
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "[1, 2, 3]").expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Give it time to process and flush output (increased from 100ms to 500ms)
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Close stdin to signal end of input
    drop(child.stdin.take());

    // Wait for process to exit gracefully
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("Stdout: {}", stdout);

    // Should return JSON-RPC invalid request error
    assert!(stdout.contains("-32600") || stdout.contains("Invalid Request"),
        "Should return JSON-RPC invalid request error (-32600). Stdout: {}", stdout);

    println!("✓ Invalid JSON-RPC structure handled correctly");
}

#[test]
#[serial]
fn test_stdio_proxy_empty_lines_ignored() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio ignores empty lines ===");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Send empty lines and valid request
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "").expect("Failed to write empty line");
        writeln!(stdin, "   ").expect("Failed to write whitespace");
        writeln!(stdin, "").expect("Failed to write empty line");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Give it time to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Terminate the child process
    child.kill().expect("Failed to kill child process");
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Empty lines should be skipped - no output expected
    assert!(stdout.trim().is_empty() || !stdout.contains("error"),
        "Empty lines should be silently ignored");

    println!("✓ Empty lines are correctly ignored");
}

#[test]
#[serial]
#[ignore] // Requires backend to be stopped to test connection failure
fn test_stdio_proxy_backend_connection_failure() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio with backend connection failure ===");

    // Use a non-existent backend URL
    std::env::set_var("MCP_BACKEND_URL", "http://localhost:65535/mcp");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:65535/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Send valid JSON-RPC request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "{}", request.to_string()).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Give it time to timeout
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Terminate the child process
    child.kill().expect("Failed to kill child process");
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Stdout: {}", stdout);
    println!("Stderr: {}", stderr);

    // Should return internal error
    assert!(stdout.contains("-32603") || stdout.contains("Internal error"),
        "Should return JSON-RPC internal error (-32603)");
    assert!(stderr.contains("Error forwarding request"),
        "Should log connection error to stderr");

    println!("✓ Backend connection failure handled correctly");

    // Restore for other tests
    setup_test_env();
}

#[test]
#[serial]
fn test_stdio_proxy_command_line_args_override_env() {
    println!("=== Testing rustymail-mcp-stdio command-line args override env vars ===");

    // Set env vars
    setup_test_env();

    // Override with command-line args
    let output = Command::new(get_binary_path())
        .args(&[
            "--backend-url", "http://custom:8080/mcp",
            "--timeout", "60"
        ])
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")  // Set env var so binary doesn't panic
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Give it time to start and print config to stderr
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Kill and get output
    let mut child = output;
    child.kill().expect("Failed to kill child");
    let output = child.wait_with_output().expect("Failed to wait for child");

    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Stderr: {}", stderr);

    // Verify command-line args were used
    assert!(stderr.contains("http://custom:8080/mcp") || stderr.contains("custom:8080"),
        "Should use command-line backend URL");
    assert!(stderr.contains("60s") || stderr.contains("Timeout: 60"),
        "Should use command-line timeout");

    println!("✓ Command-line args correctly override environment variables");
}

#[test]
#[serial]
fn test_stdio_proxy_eof_handling() {
    setup_test_env();
    println!("=== Testing rustymail-mcp-stdio EOF handling ===");

    let mut child = Command::new(get_binary_path())
        .env("MCP_BACKEND_URL", "http://localhost:9437/mcp")
        .env("MCP_TIMEOUT", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    // Close stdin immediately (send EOF)
    drop(child.stdin.take());

    // Wait for graceful exit
    let output = child.wait_with_output().expect("Failed to wait for child");
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Stderr: {}", stderr);

    // Should exit gracefully
    assert!(output.status.success() || stderr.contains("EOF"),
        "Should handle EOF gracefully");

    println!("✓ EOF handled gracefully with clean exit");
}
