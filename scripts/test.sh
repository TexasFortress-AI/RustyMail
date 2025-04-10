#!/bin/bash

# Main test script for RustyMail
# Usage: ./scripts/test.sh [unit|integration|e2e|all]

set -e

# Default to running all tests if no argument provided
TEST_TYPE=${1:-all}

# Function to run unit tests
run_unit_tests() {
    echo "Running unit tests..."
    cargo test --test unit -- --nocapture
}

# Function to run integration tests
run_integration_tests() {
    echo "Running integration tests..."
    cargo test --test integration -- --nocapture
}

# Function to run end-to-end tests
run_e2e_tests() {
    echo "Running end-to-end tests..."
    cargo test --test e2e -- --nocapture
}

# Run tests based on argument
case $TEST_TYPE in
    unit)
        run_unit_tests
        ;;
    integration)
        run_integration_tests
        ;;
    e2e)
        run_e2e_tests
        ;;
    all)
        run_unit_tests
        run_integration_tests
        run_e2e_tests
        ;;
    *)
        echo "Usage: $0 [unit|integration|e2e|all]"
        exit 1
        ;;
esac

echo "Tests completed successfully!" 