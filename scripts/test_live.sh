#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Running Live Integration Tests ---"
# Requires .env file with IMAP credentials
# Ensure RUSTYMAIL_IMAP_* variables are set
cargo test --test rest_live_test --all-features --features live_tests -- --nocapture

EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ]; then
    echo "--- Live Integration Tests Passed ---"
else
    echo "--- Live Integration Tests Failed ---"
    exit $EXIT_CODE
fi 