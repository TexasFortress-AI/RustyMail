#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Running Unit Tests ---"
cargo test --lib -- --color always
echo "--- Unit Tests Passed ---" 