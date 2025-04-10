#!/bin/bash

# End-to-end test script for RustyMail
# Usage: ./scripts/test-e2e.sh

set -e

echo "Running end-to-end tests..."
cargo test --test e2e -- --nocapture

echo "End-to-end tests completed successfully!" 