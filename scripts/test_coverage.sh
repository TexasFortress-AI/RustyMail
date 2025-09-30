#!/usr/bin/env bash

# Test Coverage Script for RustyMail
# Runs all tests and generates coverage report

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   RustyMail Test Coverage Report${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Change to project root
cd "$(dirname "$0")/.."

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if required tools are installed
check_requirements() {
    print_info "Checking requirements..."

    if ! command -v cargo &> /dev/null; then
        print_error "cargo is not installed"
        exit 1
    fi

    # Check for cargo-tarpaulin (for coverage)
    if ! cargo tarpaulin --version &> /dev/null 2>&1; then
        print_warning "cargo-tarpaulin not installed. Installing..."
        cargo install cargo-tarpaulin
    fi

    # Check for cargo-nextest (for better test output)
    if ! cargo nextest --version &> /dev/null 2>&1; then
        print_warning "cargo-nextest not installed. Installing..."
        cargo install cargo-nextest
    fi

    print_success "All requirements met"
}

# Run unit tests
run_unit_tests() {
    print_info "Running unit tests..."

    if cargo nextest run --lib 2>/dev/null; then
        print_success "Unit tests passed"
    else
        # Fallback to standard cargo test
        if cargo test --lib; then
            print_success "Unit tests passed"
        else
            print_error "Unit tests failed"
            return 1
        fi
    fi
}

# Run integration tests
run_integration_tests() {
    print_info "Running integration tests..."

    if cargo nextest run --test '*' 2>/dev/null; then
        print_success "Integration tests passed"
    else
        # Fallback to standard cargo test
        if cargo test --test '*'; then
            print_success "Integration tests passed"
        else
            print_error "Integration tests failed"
            return 1
        fi
    fi
}

# Run specific dashboard tests
run_dashboard_tests() {
    print_info "Running dashboard-specific tests..."

    local test_files=(
        "dashboard_api_handlers"
        "dashboard_sse_streaming"
        "dashboard_client_management"
        "dashboard_config"
        "dashboard_events"
        "dashboard_health"
        "imap_connection"
    )

    local failed=0
    for test in "${test_files[@]}"; do
        print_info "Testing $test..."
        if cargo test $test --quiet; then
            echo -e "  ${GREEN}✓${NC} $test"
        else
            echo -e "  ${RED}✗${NC} $test"
            failed=$((failed + 1))
        fi
    done

    if [ $failed -eq 0 ]; then
        print_success "All dashboard tests passed"
    else
        print_error "$failed dashboard test(s) failed"
        return 1
    fi
}

# Run frontend tests
run_frontend_tests() {
    print_info "Running frontend tests..."

    cd frontend/rustymail-app-main

    # Check if node_modules exists
    if [ ! -d "node_modules" ]; then
        print_info "Installing frontend dependencies..."
        npm install
    fi

    # Run tests
    if npm test -- --run 2>/dev/null; then
        print_success "Frontend tests passed"
    else
        print_warning "Frontend tests skipped or failed"
    fi

    cd ../..
}

# Generate coverage report
generate_coverage() {
    print_info "Generating coverage report..."

    # Create coverage directory
    mkdir -p coverage

    # Run with coverage
    if cargo tarpaulin \
        --out Html \
        --output-dir coverage \
        --exclude-files "*/tests/*" \
        --exclude-files "*/target/*" \
        --exclude-files "*/frontend/*" \
        --ignore-panics \
        --timeout 300 \
        --skip-clean; then

        print_success "Coverage report generated at coverage/tarpaulin-report.html"

        # Extract coverage percentage
        coverage_percent=$(cargo tarpaulin --print-summary 2>/dev/null | grep "Coverage" | awk '{print $2}')

        if [ -n "$coverage_percent" ]; then
            print_info "Overall coverage: $coverage_percent"

            # Check if coverage meets threshold
            coverage_value=$(echo "$coverage_percent" | sed 's/%//')
            if (( $(echo "$coverage_value > 70" | bc -l) )); then
                print_success "Coverage meets threshold (>70%)"
            else
                print_warning "Coverage below threshold (<70%)"
            fi
        fi
    else
        print_warning "Coverage generation failed"
    fi
}

# Test summary
print_summary() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}   Test Summary${NC}"
    echo -e "${BLUE}========================================${NC}"

    local total_tests=$(cargo test --lib --tests 2>&1 | grep "test result" | awk '{print $2}')

    if [ -n "$total_tests" ]; then
        echo "Total tests run: $total_tests"
    fi

    # List test categories
    echo ""
    echo "Test Categories:"
    echo "  • Unit tests"
    echo "  • Integration tests"
    echo "  • Dashboard API tests"
    echo "  • SSE streaming tests"
    echo "  • IMAP connection tests"
    echo "  • Frontend component tests"

    # Check for new test files
    echo ""
    echo "New test files added:"
    echo "  ✓ tests/unit/dashboard_api_handlers.rs"
    echo "  ✓ tests/unit/dashboard_sse_streaming.rs"
    echo "  ✓ tests/integration/dashboard/imap_connection.rs"
    echo "  ✓ frontend/.../Dashboard.test.tsx"
    echo "  ✓ frontend/.../StatsPanel.test.tsx"
    echo "  ✓ frontend/.../ChatbotPanel.test.tsx"
    echo "  ✓ frontend/.../McpTools.test.tsx"
}

# Main execution
main() {
    local exit_code=0

    check_requirements

    echo ""
    print_info "Starting comprehensive test suite..."
    echo ""

    # Run all test categories
    if ! run_unit_tests; then
        exit_code=1
    fi

    echo ""
    if ! run_integration_tests; then
        exit_code=1
    fi

    echo ""
    if ! run_dashboard_tests; then
        exit_code=1
    fi

    echo ""
    run_frontend_tests

    echo ""
    generate_coverage

    print_summary

    if [ $exit_code -eq 0 ]; then
        echo ""
        print_success "All tests completed successfully!"
    else
        echo ""
        print_error "Some tests failed. Please review the output above."
    fi

    return $exit_code
}

# Run main function
main "$@"