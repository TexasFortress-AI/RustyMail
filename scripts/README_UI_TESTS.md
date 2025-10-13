# RustyMail UI End-to-End Tests

## Overview

This directory contains end-to-end UI tests for the RustyMail dashboard. These tests use the Puppeteer MCP server available in Claude Code to automate browser interactions.

## Prerequisites

1. **Backend Server Running**:
   ```bash
   cargo run --bin rustymail-server
   # Should be listening on http://localhost:9437
   ```

2. **Frontend Running**:
   ```bash
   cd frontend && npm run dev
   # Should be serving on http://localhost:5173
   ```

3. **Puppeteer MCP Server**: Already configured in Claude Code's `.mcp.json`

## Running Tests

### Option 1: Manual Testing via Claude Code

Since the Puppeteer MCP server is already running in Claude Code, you can manually test the UI by asking Claude to:

1. Navigate to the dashboard
2. Take screenshots
3. Click elements
4. Fill forms
5. Verify expected behavior

Example prompts:
```
Navigate to http://localhost:5173 and take a screenshot
Click on the Email Assistant widget
Fill in the query "show me emails from today"
Verify the response appears
```

### Option 2: Automated Test Script

Run the Python test script:
```bash
python scripts/test_ui_e2e.py --url http://localhost:5173
```

This script provides a test framework but uses placeholder implementations. To make it functional:
- Integrate with Puppeteer MCP tools via Claude Code
- Or use Playwright/Puppeteer Python bindings directly

## Test Scenarios

### 1. Dashboard Loading (Task 69.4)
- Navigate to http://localhost:5173
- Verify page loads without errors
- Check console for JavaScript errors
- Verify main widgets are visible

### 2. Email Assistant Widget (Task 69.5)
- Locate Email Assistant widget
- Enter query: "show me emails from today"
- Verify query submission
- Check response appears
- Verify tool calls are logged

### 3. MCP Tools Widget (Task 69.5)
- Locate MCP Tools widget
- Select tool: `list_folders`
- Verify tool parameters display
- Execute tool
- Verify results display correctly

### 4. Account Dropdown (Task 69.5)
- Click account dropdown
- Verify accounts list displays
- Select different account
- Verify UI updates

### 5. Complete Email Workflow (Task 69.6)
- Search for emails via Email Assistant
- View email details
- Move email to different folder
- Verify email moved successfully
- Check email appears in new folder

## Integration with CI/CD (Task 69.7)

To integrate with CI/CD:

1. **Set up headless browser**:
   ```bash
   npm install -g playwright
   playwright install chromium
   ```

2. **Configure test environment**:
   ```bash
   export RUSTYMAIL_TEST_MODE=true
   export DASHBOARD_URL=http://localhost:5173
   export BACKEND_URL=http://localhost:9437
   ```

3. **Run in CI pipeline**:
   ```yaml
   # .github/workflows/e2e-tests.yml
   - name: Start backend
     run: cargo run --bin rustymail-server &

   - name: Start frontend
     run: cd frontend && npm run dev &

   - name: Wait for services
     run: sleep 10

   - name: Run E2E tests
     run: python scripts/test_ui_e2e.py
   ```

## Current Test Coverage

- ✅ MCP protocol tests (Task 68.1, 69.3)
- ✅ MCP stdio proxy tests (Task 68.2)
- ✅ MCP Python client tests (Task 69.1, 69.2)
- ⏸️ Dashboard UI tests (Tasks 69.4-69.7)

## Notes

- Puppeteer MCP server already has many screenshots from previous manual testing
- Test framework is in place but needs full implementation
- Consider using existing screenshots for regression testing
- UI tests can be run manually via Claude Code interactions
