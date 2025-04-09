#!/bin/bash
# Dashboard SSE Testing Script
# Run from the project root directory

# Text colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}==== RustyMail Dashboard SSE Tests ====${NC}"
echo "Starting test suite at $(date)"
echo

# Ensure no stray servers are running
echo -e "${YELLOW}Checking for stray server processes...${NC}"
SERVER_PIDS=$(lsof -t -i :3000 -i :8080 2>/dev/null)
if [ ! -z "$SERVER_PIDS" ]; then
    echo "Found stray server processes: $SERVER_PIDS"
    echo "Terminating these processes..."
    for PID in $SERVER_PIDS; do
        kill -9 $PID 2>/dev/null
        echo "Killed process $PID"
    done
else
    echo "No stray server processes found"
fi

# Clean up any PID files
echo -e "${YELLOW}Cleaning up any stale PID files...${NC}"
mkdir -p ./tmp
rm -f ./tmp/rustymail-test-*.pid 2>/dev/null

# Build the project
echo -e "${YELLOW}Building project...${NC}"
cargo build --features integration_tests
if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed${NC}"
    exit 1
fi

# Create results directory
RESULT_DIR="test_results/dashboard"
mkdir -p $RESULT_DIR
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RESULT_FILE="$RESULT_DIR/test_results_$TIMESTAMP.log"

echo -e "${YELLOW}Running dashboard SSE tests...${NC}"
echo "Results will be saved to $RESULT_FILE"
RUST_BACKTRACE=1 cargo test --test dashboard_sse_test --features integration_tests -- --test-threads=1 --nocapture 2>&1 | tee $RESULT_FILE
TEST_RESULT=$?

# Check for any orphaned test processes
echo -e "${YELLOW}Checking for orphaned test processes...${NC}"
SERVER_PIDS=$(lsof -t -i :3000 -i :8080 2>/dev/null)
if [ ! -z "$SERVER_PIDS" ]; then
    echo "Found orphaned server processes: $SERVER_PIDS"
    echo "Terminating these processes..."
    for PID in $SERVER_PIDS; do
        kill -9 $PID 2>/dev/null
        echo "Killed process $PID"
    done
else
    echo "No orphaned server processes found"
fi

# Extract summary statistics
SUCCESS_COUNT=$(grep -c "test result: ok" $RESULT_FILE)
FAIL_COUNT=$(grep -c "test result: FAILED" $RESULT_FILE)

echo
echo -e "${YELLOW}=== Test Summary ===${NC}"
echo -e "Successful test runs: ${GREEN}$SUCCESS_COUNT${NC}"
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "Failed test runs: ${RED}$FAIL_COUNT${NC}"
else
    echo -e "Failed test runs: ${GREEN}$FAIL_COUNT${NC}"
fi
echo
echo -e "Tests completed at $(date)"

if [ $TEST_RESULT -ne 0 ]; then
    echo -e "${RED}Some tests failed. Please check the log for details.${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi 