#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Running Live Integration Tests ---"
# Requires .env file with IMAP credentials
# Ensure RUSTYMAIL_IMAP_* variables are set
cargo test --features integration_tests,live_tests --test rest_live_test -- --ignored --show-output
echo "--- Live Integration Tests Passed ---" 