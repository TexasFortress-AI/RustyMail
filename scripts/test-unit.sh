#!/bin/bash

# Unit test script for RustyMail
# Usage: ./scripts/test-unit.sh

set -e

echo "Running unit tests..."
cargo test --test unit -- --nocapture

echo "Unit tests completed successfully!" 