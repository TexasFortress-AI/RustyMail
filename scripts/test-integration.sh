#!/bin/bash

# Integration test script for RustyMail
# Usage: ./scripts/test-integration.sh

set -e

echo "Running integration tests..."
cargo test --test integration -- --nocapture

echo "Integration tests completed successfully!" 