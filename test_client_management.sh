#!/bin/bash
# Simple test script for client management functionality

echo "Testing Client Management Service..."

# Create a simple Rust test program
cat > /tmp/test_client_mgmt.rs << 'EOF'
use rustymail::dashboard::services::clients::ClientManager;
use rustymail::dashboard::api::models::{ClientType, ClientStatus};
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("Testing Client Management...");

    let manager = ClientManager::new(Duration::from_secs(60));

    // Test 1: Register client
    println!("Test 1: Registering client...");
    let client_id = manager.register_client(
        ClientType::Sse,
        Some("127.0.0.1".to_string()),
        Some("TestAgent/1.0".to_string()),
    ).await;
    assert!(!client_id.is_empty());
    println!("✓ Client registered with ID: {}", client_id);

    // Test 2: Get client count
    println!("Test 2: Checking client count...");
    let count = manager.get_client_count().await;
    assert_eq!(count, 1);
    println!("✓ Client count is correct: {}", count);

    // Test 3: Update activity
    println!("Test 3: Updating client activity...");
    manager.update_client_activity(&client_id).await;
    println!("✓ Activity updated");

    // Test 4: Get clients list
    println!("Test 4: Getting clients list...");
    let clients = manager.get_clients(1, 10, None).await;
    assert_eq!(clients.pagination.total, 1);
    assert_eq!(clients.clients[0].id, client_id);
    println!("✓ Client found in list");

    // Test 5: Remove client
    println!("Test 5: Removing client...");
    manager.remove_client(&client_id).await;
    let count = manager.get_client_count().await;
    assert_eq!(count, 0);
    println!("✓ Client removed successfully");

    println!("\nAll tests passed! ✅");
}
EOF

# Compile and run the test
echo "Compiling test..."
rustc /tmp/test_client_mgmt.rs \
    --edition 2021 \
    --extern rustymail=target/debug/librustymail.rlib \
    --extern tokio=target/debug/deps/libtokio-*.rlib \
    -L target/debug/deps \
    -o /tmp/test_client_mgmt 2>&1

if [ $? -eq 0 ]; then
    echo "Running test..."
    /tmp/test_client_mgmt
else
    echo "Compilation failed"
    exit 1
fi