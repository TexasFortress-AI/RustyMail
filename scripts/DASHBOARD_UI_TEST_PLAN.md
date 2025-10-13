# RustyMail Dashboard UI Test Plan

## Overview

This document outlines the comprehensive end-to-end test plan for the RustyMail dashboard UI (Task 69.5). Tests validate Email Assistant widget, MCP Tools widget, real-time SSE updates, and multi-account functionality.

## Test Environment Setup

### Prerequisites
1. Backend server running on http://localhost:9437
2. Frontend running on http://localhost:5173
3. Puppeteer MCP server available in Claude Code
4. Test account with sample emails configured

### Test Data Requirements
- At least 2 email accounts configured
- Sample emails in multiple folders (INBOX, Sent, Drafts)
- Emails with various dates for date-based filtering
- Emails with different subjects/senders for search testing

## Test Suite 1: Dashboard Loading & Initial State

### Test 1.1: Page Load
**Steps:**
1. Navigate to http://localhost:5173
2. Wait for page to fully load (check for loading spinner removal)
3. Verify no JavaScript console errors

**Expected:**
- Page loads within 3 seconds
- No console errors
- Dashboard layout renders correctly

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_navigate(url="http://localhost:5173")
mcp__puppeteer__puppeteer_screenshot(name="dashboard-initial-load")
```

### Test 1.2: Widget Visibility
**Steps:**
1. Check Email Assistant widget is visible
2. Check MCP Tools widget is visible
3. Verify dashboard metrics display

**Expected:**
- Both widgets visible and properly laid out
- Metrics show connection count and email count
- Account dropdown shows current account

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_evaluate(script="""
  const assistant = document.querySelector('[data-testid=\"email-assistant\"]');
  const tools = document.querySelector('[data-testid=\"mcp-tools\"]');
  return {
    assistantVisible: assistant !== null,
    toolsVisible: tools !== null
  };
""")
```

## Test Suite 2: Email Assistant Widget

### Test 2.1: Simple Query
**Steps:**
1. Locate Email Assistant input field
2. Enter query: "show me emails from today"
3. Click submit button
4. Wait for response

**Expected:**
- Query submits successfully
- Response appears within 5 seconds
- Response shows tool call to `search_emails`
- Email results display correctly

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_fill(selector="input[name='query']", value="show me emails from today")
mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
time.sleep(2)
mcp__puppeteer__puppeteer_screenshot(name="assistant-query-response")
```

### Test 2.2: Complex Query with Multiple Tools
**Steps:**
1. Enter query: "move all unread emails to a folder called Archive"
2. Submit and wait for response

**Expected:**
- Response shows tool call sequence:
  1. `search_emails` (finding unread emails)
  2. `atomic_batch_move` (moving to Archive folder)
- Success message displayed
- Tool call log shows all invocations

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_fill(selector="input[name='query']", value="move all unread emails to Archive")
mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
time.sleep(5)
mcp__puppeteer__puppeteer_screenshot(name="assistant-complex-query")
```

### Test 2.3: Error Handling
**Steps:**
1. Enter invalid query: "delete everything permanently"
2. Submit and wait for response

**Expected:**
- Error message displayed appropriately
- No actual deletion occurs
- UI remains stable

### Test 2.4: Real-time Streaming
**Steps:**
1. Enter query that requires multiple tool calls
2. Watch response stream in real-time

**Expected:**
- Response text streams character-by-character
- Tool calls show as they execute
- Progress indicators update live

## Test Suite 3: MCP Tools Widget

### Test 3.1: Tool List Display
**Steps:**
1. Locate MCP Tools widget
2. Verify tool dropdown/list is visible
3. Check all 18 tools are present

**Expected:**
- All 18 tools visible:
  * list_folders, list_folders_hierarchical
  * search_emails, fetch_emails_with_mime
  * atomic_move_message, atomic_batch_move
  * mark_as_deleted, delete_messages, undelete_messages
  * expunge, list_cached_emails
  * get_email_by_uid, get_email_by_index
  * count_emails_in_folder, get_folder_stats
  * search_cached_emails, list_accounts, set_current_account

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_evaluate(script="""
  const toolSelect = document.querySelector('select[name=\"tool\"]');
  const options = Array.from(toolSelect.options).map(o => o.value);
  return options;
""")
```

### Test 3.2: Simple Tool Execution (list_folders)
**Steps:**
1. Select tool: `list_folders`
2. Verify no parameters required (or account_id populated)
3. Click "Execute" button
4. Wait for results

**Expected:**
- Tool executes successfully
- Results show folder list in JSON format
- Response time < 2 seconds

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_click(selector="select[name='tool']")
mcp__puppeteer__puppeteer_select(selector="select[name='tool']", value="list_folders")
mcp__puppeteer__puppeteer_click(selector="button[data-action='execute-tool']")
time.sleep(1)
mcp__puppeteer__puppeteer_screenshot(name="tools-list-folders-result")
```

### Test 3.3: Parameterized Tool Execution (search_emails)
**Steps:**
1. Select tool: `search_emails`
2. Fill parameters:
   - folder_name: "INBOX"
   - query: "subject:test"
3. Click "Execute"
4. Verify results

**Expected:**
- Parameter inputs appear for folder_name and query
- Tool executes with provided parameters
- Results show matching emails
- JSON response properly formatted

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_select(selector="select[name='tool']", value="search_emails")
mcp__puppeteer__puppeteer_fill(selector="input[name='folder_name']", value="INBOX")
mcp__puppeteer__puppeteer_fill(selector="input[name='query']", value="subject:test")
mcp__puppeteer__puppeteer_click(selector="button[data-action='execute-tool']")
time.sleep(2)
mcp__puppeteer__puppeteer_screenshot(name="tools-search-result")
```

### Test 3.4: Tool Execution Error Handling
**Steps:**
1. Select tool: `get_email_by_uid`
2. Enter invalid UID: "999999"
3. Execute tool

**Expected:**
- Error message displays clearly
- Error details shown (e.g., "Email not found")
- UI remains stable and responsive

## Test Suite 4: Multi-Account Functionality

### Test 4.1: Account Dropdown Display
**Steps:**
1. Locate account dropdown in header
2. Click to open dropdown
3. Verify all configured accounts listed

**Expected:**
- Dropdown opens smoothly
- All accounts displayed with email addresses
- Current account highlighted

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_click(selector="button[aria-label='account-selector']")
time.sleep(0.5)
mcp__puppeteer__puppeteer_screenshot(name="account-dropdown-open")
```

### Test 4.2: Account Switching
**Steps:**
1. Open account dropdown
2. Select different account
3. Wait for UI to update

**Expected:**
- Account switches successfully
- Email count updates for new account
- Folder list updates (if different)
- Both widgets reflect new account context

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_click(selector="button[aria-label='account-selector']")
mcp__puppeteer__puppeteer_click(selector="li[data-account-id='account2']")
time.sleep(1)
mcp__puppeteer__puppeteer_screenshot(name="account-switched")
```

### Test 4.3: Cross-Account Tool Execution
**Steps:**
1. Switch to account 1
2. Execute `count_emails_in_folder` for INBOX
3. Switch to account 2
4. Execute same tool
5. Verify results are different (account-specific)

**Expected:**
- Tool results properly scoped to selected account
- No data leakage between accounts
- Counts match actual email data

## Test Suite 5: Real-Time Updates (SSE)

### Test 5.1: Connection Status Indicator
**Steps:**
1. Check initial connection status indicator
2. Verify shows "Connected" or similar

**Expected:**
- Status indicator visible
- Shows connected state
- Connection count increments when new client connects

### Test 5.2: Live Email Count Updates
**Steps:**
1. Note current email count in dashboard
2. Use MCP Tools to move/delete emails
3. Verify count updates without page refresh

**Expected:**
- Dashboard metrics update in real-time via SSE
- No page refresh required
- Update appears within 1-2 seconds

**Implementation Note:**
This requires backend to emit SSE events when email operations occur. May need backend enhancement for full implementation.

### Test 5.3: SSE Reconnection
**Steps:**
1. Monitor network tab for SSE connection
2. Briefly disconnect backend server
3. Reconnect backend server
4. Verify frontend reconnects automatically

**Expected:**
- Frontend detects disconnection
- Shows "Reconnecting..." indicator
- Reconnects within 5 seconds
- Resumes normal operation

## Test Suite 6: Visual Regression & Responsive Design

### Test 6.1: Desktop Layout
**Steps:**
1. Set browser viewport to 1920x1080
2. Take screenshot
3. Verify layout matches design specs

**Expected:**
- Widgets side-by-side or stacked per design
- No horizontal scrolling
- All elements visible

**Puppeteer Commands:**
```python
mcp__puppeteer__puppeteer_navigate(url="http://localhost:5173")
# Set viewport via evaluate if needed
mcp__puppeteer__puppeteer_screenshot(name="desktop-layout", width=1920, height=1080)
```

### Test 6.2: Mobile Layout
**Steps:**
1. Set viewport to 375x667 (iPhone SE)
2. Take screenshot
3. Verify responsive layout

**Expected:**
- Widgets stack vertically
- All controls accessible
- Text readable without zooming

### Test 6.3: Visual Regression Comparison
**Steps:**
1. Compare current screenshots with baseline
2. Flag any unexpected visual changes

**Expected:**
- No unintended visual regressions
- Layout consistent with previous version

## Test Suite 7: Performance & Load Testing

### Test 7.1: Initial Load Performance
**Steps:**
1. Clear browser cache
2. Navigate to dashboard
3. Measure time to interactive (TTI)

**Expected:**
- TTI < 3 seconds
- First Contentful Paint < 1 second
- No layout shift (CLS < 0.1)

### Test 7.2: Tool Execution Performance
**Steps:**
1. Execute `search_emails` with broad query
2. Measure response time

**Expected:**
- Response time < 2 seconds for typical query
- UI remains responsive during execution
- Progress indicator shows during long operations

### Test 7.3: Concurrent Tool Executions
**Steps:**
1. Execute multiple tools simultaneously
2. Verify all complete successfully

**Expected:**
- All tools execute without errors
- No race conditions or data corruption
- UI updates correctly for all results

## Automation Implementation Notes

### Using Puppeteer MCP Server in Claude Code

The Puppeteer MCP server provides these tools:
- `mcp__puppeteer__puppeteer_navigate(url)` - Navigate to URL
- `mcp__puppeteer__puppeteer_screenshot(name)` - Take screenshot
- `mcp__puppeteer__puppeteer_click(selector)` - Click element
- `mcp__puppeteer__puppeteer_fill(selector, value)` - Fill input
- `mcp__puppeteer__puppeteer_select(selector, value)` - Select option
- `mcp__puppeteer__puppeteer_hover(selector)` - Hover element
- `mcp__puppeteer__puppeteer_evaluate(script)` - Run JavaScript

### Test Execution Options

**Option 1: Manual via Claude Code**
Ask Claude to execute test steps using Puppeteer MCP tools directly.

**Option 2: Python Script**
Extend `test_ui_e2e.py` to call Puppeteer tools programmatically (requires MCP client integration).

**Option 3: Playwright/Puppeteer Native**
Use Playwright Python bindings directly without MCP layer for CI/CD.

## Success Criteria

Task 69.5 is complete when:
- [ ] All 7 test suites documented
- [ ] Test automation scripts created
- [ ] All tests pass on local environment
- [ ] Screenshot artifacts collected
- [ ] Test results documented
- [ ] CI/CD integration instructions provided

## Related Tasks

- Task 69.4: Puppeteer MCP server configuration ✅
- Task 69.6: Complete workflow integration tests (next)
- Task 69.7: CI/CD integration (final)
