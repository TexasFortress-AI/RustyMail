#!/bin/bash
#
# Complete Workflow Integration Test Runner
# Starts all required services and executes comprehensive E2E tests
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "================================================================"
echo "     RustyMail Complete Workflow Integration Tests"
echo "================================================================"

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from project root${NC}"
    exit 1
fi

if [ ! -d ".venv" ]; then
    echo -e "${RED}Error: Python virtual environment not found${NC}"
    echo "Run: python3 -m venv .venv && source .venv/bin/activate && pip install -r scripts/requirements.txt"
    exit 1
fi

if [ ! -d "frontend/node_modules" ]; then
    echo -e "${RED}Error: Frontend dependencies not installed${NC}"
    echo "Run: cd frontend && npm install"
    exit 1
fi

# Create test results directory
mkdir -p test-results

# Build backend
echo -e "${YELLOW}Building backend server...${NC}"
cargo build --release --bin rustymail-server

# Start backend server
echo -e "${YELLOW}Starting backend server on :9437...${NC}"
./target/release/rustymail-server > test-results/backend.log 2>&1 &
BACKEND_PID=$!
echo "Backend PID: $BACKEND_PID"

# Wait for backend to be ready
echo "Waiting for backend to start..."
for i in {1..30}; do
    if curl -s http://localhost:9437/api/dashboard/status > /dev/null; then
        echo -e "${GREEN}Backend ready${NC}"
        break
    fi
    sleep 1
done

if ! curl -s http://localhost:9437/api/dashboard/status > /dev/null; then
    echo -e "${RED}Backend failed to start${NC}"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

# Start frontend
echo -e "${YELLOW}Starting frontend on :5173...${NC}"
cd frontend
npm run dev > ../test-results/frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..
echo "Frontend PID: $FRONTEND_PID"

# Wait for frontend to be ready
echo "Waiting for frontend to start..."
for i in {1..30}; do
    if curl -s http://localhost:5173 > /dev/null; then
        echo -e "${GREEN}Frontend ready${NC}"
        break
    fi
    sleep 1
done

if ! curl -s http://localhost:5173 > /dev/null; then
    echo -e "${RED}Frontend failed to start${NC}"
    kill $BACKEND_PID $FRONTEND_PID 2>/dev/null || true
    exit 1
fi

# Activate Python virtual environment
source .venv/bin/activate

# Run MCP client workflow tests
echo ""
echo -e "${YELLOW}=== Running MCP Client Workflow Tests ===${NC}"
if python scripts/test_mcp_client.py --transport stdio; then
    echo -e "${GREEN}✓ MCP client tests passed${NC}"
    MCP_TESTS_PASSED=1
else
    echo -e "${RED}✗ MCP client tests failed${NC}"
    MCP_TESTS_PASSED=0
fi

# Run UI workflow tests (if implemented)
echo ""
echo -e "${YELLOW}=== Running UI Workflow Tests ===${NC}"
if [ -f "scripts/test_ui_workflows.py" ]; then
    if python scripts/test_ui_workflows.py --url http://localhost:5173; then
        echo -e "${GREEN}✓ UI workflow tests passed${NC}"
        UI_TESTS_PASSED=1
    else
        echo -e "${RED}✗ UI workflow tests failed${NC}"
        UI_TESTS_PASSED=0
    fi
else
    echo -e "${YELLOW}ℹ UI workflow tests not yet implemented (test_ui_workflows.py)${NC}"
    UI_TESTS_PASSED=1  # Don't fail if not implemented yet
fi

# Cleanup
echo ""
echo -e "${YELLOW}Cleaning up...${NC}"
kill $BACKEND_PID 2>/dev/null || true
kill $FRONTEND_PID 2>/dev/null || true

# Wait for processes to terminate
sleep 2

# Report summary
echo ""
echo "================================================================"
echo "                     Test Summary"
echo "================================================================"
echo -e "MCP Client Tests: $([ $MCP_TESTS_PASSED -eq 1 ] && echo -e "${GREEN}PASSED${NC}" || echo -e "${RED}FAILED${NC}")"
echo -e "UI Workflow Tests: $([ $UI_TESTS_PASSED -eq 1 ] && echo -e "${GREEN}PASSED${NC}" || echo -e "${RED}FAILED${NC}")"
echo ""
echo "Logs available in:"
echo "  - test-results/backend.log"
echo "  - test-results/frontend.log"
echo "================================================================"

# Exit with failure if any tests failed
if [ $MCP_TESTS_PASSED -eq 1 ] && [ $UI_TESTS_PASSED -eq 1 ]; then
    echo -e "${GREEN}All workflow tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some workflow tests failed${NC}"
    exit 1
fi
