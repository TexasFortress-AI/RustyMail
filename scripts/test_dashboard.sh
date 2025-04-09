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

echo -e "${YELLOW}Running basic SSE tests...${NC}"
echo "Results will be saved to $RESULT_FILE"
cargo test --test dashboard_sse_test --features integration_tests -- --nocapture 2>&1 | tee $RESULT_FILE
if [ $? -ne 0 ]; then
    echo -e "${RED}Basic SSE tests failed${NC}"
else
    echo -e "${GREEN}Basic SSE tests completed${NC}"
fi

echo
echo -e "${YELLOW}Running stress tests (resource intensive)...${NC}"
echo "This may take some time. Results will be appended to $RESULT_FILE"
cargo test --test dashboard_sse_test --features integration_tests -- --ignored --nocapture 2>&1 | tee -a $RESULT_FILE
if [ $? -ne 0 ]; then
    echo -e "${RED}Stress tests failed${NC}"
else
    echo -e "${GREEN}Stress tests completed${NC}"
fi

echo
echo -e "${YELLOW}All tests completed at $(date)${NC}"
echo -e "See detailed results in ${GREEN}$RESULT_FILE${NC}"

# Extract summary statistics
SUCCESS_COUNT=$(grep -c "test result: ok" $RESULT_FILE)
FAIL_COUNT=$(grep -c "test result: FAILED" $RESULT_FILE)

echo
echo -e "${YELLOW}=== Test Summary ===${NC}"
echo -e "Successful test runs: ${GREEN}$SUCCESS_COUNT${NC}"
echo -e "Failed test runs: ${RED}$FAIL_COUNT${NC}"
echo

# Check if there were any failures
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}Some tests failed. Please check the log for details.${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi 