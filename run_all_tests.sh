#!/bin/bash
reset

# Exit immediately if a command exits with a non-zero status.
set -e

echo "--- Running Standard Unit & Integration Tests ---"
echo "(Includes non-ignored tests, typically fast-running)"
cargo test --features integration_tests

echo ""
echo "--- Running Dashboard SSE Integration Tests ---"
echo "(Requires server build, uses --nocapture for logs)"
cargo test --test dashboard_sse_test --features integration_tests -- --nocapture

echo ""
echo "--- Running Ignored End-to-End REST API Tests ---"
echo "(Requires external IMAP server configured in .env, uses --nocapture)"
cargo test --test rest_e2e_test --features integration_tests -- --ignored --nocapture

echo ""
echo "--- All Tests Completed ---" 