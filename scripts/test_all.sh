#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

SCRIPT_DIR=$(dirname "$0")

echo "=== Running ALL Tests ==="

"$SCRIPT_DIR/test_unit.sh"
echo # Add a newline
"$SCRIPT_DIR/test_live.sh"
echo # Add a newline
"$SCRIPT_DIR/test_e2e.sh"

echo "=== ALL Tests Passed ===" 