#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Running E2E Tests ---"
# Requires .env file and server to be buildable/runnable
RUST_LOG=info cargo test --features integration_tests --test rest_e2e_test -- --show-output
echo "--- E2E Tests Passed ---" 