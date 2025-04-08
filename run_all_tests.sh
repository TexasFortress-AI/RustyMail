#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

echo "--- Running Standard Unit & Integration Tests ---"
cargo test --features integration_tests

echo ""
echo "--- Running Ignored End-to-End REST API Tests ---"
echo "NOTE: Ensure a compatible IMAP server is running and configured in .env"
cargo test --test rest_e2e_test --features integration_tests -- --ignored --nocapture

echo ""
echo "--- All Tests Completed ---" 