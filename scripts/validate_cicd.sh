#!/bin/bash
#
# CI/CD Readiness Validation Script
# Verifies all required files and dependencies for CI/CD pipeline
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo ""
echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}     CI/CD Readiness Validation${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

ERRORS=0

# Check required files exist
echo -e "${YELLOW}Checking required files...${NC}"

files=(
    ".github/workflows/e2e-tests.yml"
    "scripts/test_mcp_client.py"
    "scripts/test_ui_e2e.py"
    "scripts/run_workflow_tests.sh"
    "scripts/requirements.txt"
    "scripts/README_UI_TESTS.md"
    "scripts/DASHBOARD_UI_TEST_PLAN.md"
    "scripts/WORKFLOW_INTEGRATION_TESTS.md"
    "scripts/CICD_INTEGRATION.md"
    "frontend/package.json"
    "Cargo.toml"
)

for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo -e "  ${GREEN}✓${NC} Found: $file"
    else
        echo -e "  ${RED}✗${NC} Missing: $file"
        ((ERRORS++))
    fi
done

echo ""

# Check scripts are executable
echo -e "${YELLOW}Checking script permissions...${NC}"

scripts=(
    "scripts/test_mcp_client.py"
    "scripts/test_ui_e2e.py"
    "scripts/run_workflow_tests.sh"
)

for script in "${scripts[@]}"; do
    if [ -x "$script" ]; then
        echo -e "  ${GREEN}✓${NC} Executable: $script"
    else
        echo -e "  ${YELLOW}⚠${NC}  Not executable: $script (fixing...)"
        chmod +x "$script"
    fi
done

echo ""

# Check Rust builds
echo -e "${YELLOW}Checking Rust build...${NC}"

if cargo build --release --bin rustymail-server 2>&1 | grep -q "Finished"; then
    echo -e "  ${GREEN}✓${NC} Backend builds successfully"
else
    echo -e "  ${RED}✗${NC} Backend build failed"
    ((ERRORS++))
fi

if cargo build --release --bin rustymail-mcp-stdio 2>&1 | grep -q "Finished"; then
    echo -e "  ${GREEN}✓${NC} MCP stdio proxy builds successfully"
else
    echo -e "  ${RED}✗${NC} MCP stdio proxy build failed"
    ((ERRORS++))
fi

echo ""

# Check Python virtual environment
echo -e "${YELLOW}Checking Python environment...${NC}"

if [ -d ".venv" ]; then
    echo -e "  ${GREEN}✓${NC} Virtual environment exists"

    # Activate and check dependencies
    source .venv/bin/activate

    if python -c "import mcp" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} MCP SDK installed"
    else
        echo -e "  ${RED}✗${NC} MCP SDK not installed"
        echo -e "      Run: pip install -r scripts/requirements.txt"
        ((ERRORS++))
    fi

    if python -c "import dotenv" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} python-dotenv installed"
    else
        echo -e "  ${RED}✗${NC} python-dotenv not installed"
        echo -e "      Run: pip install -r scripts/requirements.txt"
        ((ERRORS++))
    fi

    deactivate
else
    echo -e "  ${RED}✗${NC} Virtual environment not found"
    echo -e "      Run: python3 -m venv .venv && source .venv/bin/activate && pip install -r scripts/requirements.txt"
    ((ERRORS++))
fi

echo ""

# Check Node.js dependencies
echo -e "${YELLOW}Checking frontend dependencies...${NC}"

if [ -d "frontend/node_modules" ]; then
    echo -e "  ${GREEN}✓${NC} Frontend dependencies installed"
else
    echo -e "  ${YELLOW}⚠${NC}  Frontend dependencies not installed"
    echo -e "      Run: cd frontend && npm install"
fi

if [ -f "frontend/package-lock.json" ]; then
    echo -e "  ${GREEN}✓${NC} package-lock.json exists"
else
    echo -e "  ${YELLOW}⚠${NC}  package-lock.json missing (should be committed)"
fi

echo ""

# Check test configuration
echo -e "${YELLOW}Checking test configuration...${NC}"

if [ -f "tests/integration/mcp_http.rs" ]; then
    echo -e "  ${GREEN}✓${NC} MCP HTTP integration tests exist"
else
    echo -e "  ${YELLOW}⚠${NC}  MCP HTTP integration tests not found"
fi

if [ -f "tests/integration/mcp_stdio.rs" ]; then
    echo -e "  ${GREEN}✓${NC} MCP stdio integration tests exist"
else
    echo -e "  ${YELLOW}⚠${NC}  MCP stdio integration tests not found"
fi

echo ""

# Check GitHub Actions syntax
echo -e "${YELLOW}Checking GitHub Actions workflow syntax...${NC}"

if command -v yamllint &> /dev/null; then
    if yamllint .github/workflows/e2e-tests.yml; then
        echo -e "  ${GREEN}✓${NC} Workflow YAML is valid"
    else
        echo -e "  ${RED}✗${NC} Workflow YAML has syntax errors"
        ((ERRORS++))
    fi
else
    echo -e "  ${YELLOW}⚠${NC}  yamllint not installed (skipping syntax check)"
    echo -e "      Install: pip install yamllint"
fi

echo ""

# Summary
echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}     Validation Summary${NC}"
echo -e "${BLUE}================================================${NC}"

if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    echo ""
    echo "CI/CD pipeline is ready to run."
    echo ""
    echo "Next steps:"
    echo "  1. Commit changes: git add . && git commit -m 'Add E2E CI/CD pipeline'"
    echo "  2. Push to GitHub: git push"
    echo "  3. Check Actions tab for workflow results"
    echo ""
    exit 0
else
    echo -e "${RED}✗ $ERRORS error(s) found${NC}"
    echo ""
    echo "Please fix the errors above before running CI/CD pipeline."
    echo ""
    exit 1
fi
