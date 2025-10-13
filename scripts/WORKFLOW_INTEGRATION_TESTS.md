# RustyMail Complete Workflow Integration Tests

## Overview

This document defines comprehensive end-to-end workflow tests that validate complete email operations from MCP client through the dashboard UI to the IMAP backend (Task 69.6). These tests ensure cross-interface consistency and proper data flow through all system layers.

## Test Architecture

```
┌─────────────────┐
│  MCP Client     │ ← Python test script using MCP SDK
│  (test_mcp_     │
│   client.py)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ rustymail-mcp-  │ ← Stdio proxy (Rust)
│ stdio           │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ HTTP Backend    │ ← /mcp endpoint (Rust)
│ :9437/mcp       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Dashboard UI    │ ← React frontend + SSE
│ :5173           │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ IMAP Backend    │ ← Email storage
│ (SQLite cache)  │
└─────────────────┘
```

## Test Workflow Categories

### Category 1: Email Search Workflows

#### Workflow 1.1: MCP Search → UI Display Verification
**Objective:** Verify that emails searched via MCP client appear correctly in dashboard UI.

**Steps:**
1. **MCP Client**: Execute `search_emails` with query "subject:test"
   ```python
   result = await session.call_tool("search_emails", {
       "folder_name": "INBOX",
       "query": "subject:test"
   })
   email_uids = [email['uid'] for email in result]
   ```

2. **Backend Verification**: Check that search results are cached in SQLite
   ```bash
   sqlite3 .rustymail/cache.db "SELECT uid FROM emails WHERE subject LIKE '%test%'"
   ```

3. **UI Verification**: Use Puppeteer to verify results display in Email Assistant
   ```python
   mcp__puppeteer__puppeteer_navigate(url="http://localhost:5173")
   mcp__puppeteer__puppeteer_fill(selector="input[name='query']",
                                   value="show me emails with subject test")
   mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
   time.sleep(2)

   # Verify response shows matching emails
   result = mcp__puppeteer__puppeteer_evaluate(script="""
       const response = document.querySelector('[data-testid=\"assistant-response\"]');
       return response ? response.textContent : null;
   """)

   # Assert: Response contains same UIDs as MCP search
   for uid in email_uids:
       assert str(uid) in result, f"UID {uid} not found in UI response"
   ```

**Expected Results:**
- ✅ MCP search returns N emails
- ✅ SQLite cache contains all N emails
- ✅ Dashboard UI displays all N emails
- ✅ Email metadata matches across all layers
- ⏱️ Total workflow time < 5 seconds

**Performance Metrics:**
- MCP search time: < 1 second
- Cache lookup time: < 100ms
- UI display time: < 2 seconds

#### Workflow 1.2: Cross-Account Search Consistency
**Objective:** Verify search results are properly scoped to current account.

**Steps:**
1. **MCP Client**: Search account 1 INBOX
   ```python
   await session.call_tool("set_current_account", {"account_id": "account1@example.com"})
   result1 = await session.call_tool("search_emails", {"folder_name": "INBOX", "query": "from:sender1"})
   ```

2. **MCP Client**: Search account 2 INBOX with same query
   ```python
   await session.call_tool("set_current_account", {"account_id": "account2@example.com"})
   result2 = await session.call_tool("search_emails", {"folder_name": "INBOX", "query": "from:sender1"})
   ```

3. **Verification**: Results should be different (account-specific)
   ```python
   assert result1 != result2, "Search results should be account-specific"
   ```

4. **UI Verification**: Switch accounts in dashboard and verify results change
   ```python
   # Select account 1 in dropdown
   mcp__puppeteer__puppeteer_click(selector="button[aria-label='account-selector']")
   mcp__puppeteer__puppeteer_click(selector="li[data-account-id='account1']")

   # Get email count
   count1 = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"email-count\"]').textContent;
   """)

   # Switch to account 2
   mcp__puppeteer__puppeteer_click(selector="button[aria-label='account-selector']")
   mcp__puppeteer__puppeteer_click(selector="li[data-account-id='account2']")

   # Get email count
   count2 = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"email-count\"]').textContent;
   """)

   assert count1 != count2, "Email counts should differ between accounts"
   ```

**Expected Results:**
- ✅ MCP search respects account context
- ✅ UI displays account-specific data
- ✅ No data leakage between accounts
- ✅ Account switching updates UI within 1 second

### Category 2: Email Move Workflows

#### Workflow 2.1: MCP Move → Backend Update → UI Refresh
**Objective:** Verify email moves via MCP propagate to UI in real-time.

**Steps:**
1. **Setup**: Identify email in INBOX
   ```python
   emails = await session.call_tool("list_cached_emails", {"folder_name": "INBOX", "limit": 1})
   test_uid = emails[0]['uid']
   ```

2. **MCP Client**: Move email to Trash
   ```python
   result = await session.call_tool("atomic_move_message", {
       "source_folder": "INBOX",
       "dest_folder": "Trash",
       "uid": test_uid
   })
   assert result['success'] == True
   ```

3. **Backend Verification**: Check SQLite cache updated
   ```bash
   # Email should now show folder_name = "Trash"
   sqlite3 .rustymail/cache.db \
     "SELECT folder_name FROM emails WHERE uid = ${test_uid}"
   ```

4. **UI Verification**: Check Email Assistant reflects change
   ```python
   # Query for email in INBOX (should not find it)
   mcp__puppeteer__puppeteer_fill(selector="input[name='query']",
                                   value=f"find email with UID {test_uid} in INBOX")
   mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
   time.sleep(1)

   response = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"assistant-response\"]').textContent;
   """)

   assert "not found" in response.lower() or "no emails" in response.lower()

   # Query for email in Trash (should find it)
   mcp__puppeteer__puppeteer_fill(selector="input[name='query']",
                                   value=f"find email with UID {test_uid} in Trash")
   mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
   time.sleep(1)

   response = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"assistant-response\"]').textContent;
   """)

   assert str(test_uid) in response, "Email should be found in Trash"
   ```

5. **SSE Verification**: Check that dashboard metrics updated via SSE
   ```python
   # Email count in dashboard should reflect the move
   inbox_count = mcp__puppeteer__puppeteer_evaluate(script="""
       const metrics = document.querySelector('[data-testid=\"folder-metrics\"]');
       const inboxMetric = Array.from(metrics.querySelectorAll('div'))
                               .find(d => d.textContent.includes('INBOX'));
       return inboxMetric ? parseInt(inboxMetric.querySelector('.count').textContent) : 0;
   """)

   trash_count = mcp__puppeteer__puppeteer_evaluate(script="""
       const metrics = document.querySelector('[data-testid=\"folder-metrics\"]');
       const trashMetric = Array.from(metrics.querySelectorAll('div'))
                               .find(d => d.textContent.includes('Trash'));
       return trashMetric ? parseInt(trashMetric.querySelector('.count').textContent) : 0;
   """)

   # Verify counts are consistent with move operation
   ```

**Expected Results:**
- ✅ MCP move operation succeeds
- ✅ SQLite cache updates immediately
- ✅ UI reflects move within 2 seconds via SSE
- ✅ Email count metrics update correctly
- ⏱️ Total workflow time < 3 seconds

#### Workflow 2.2: Batch Move via MCP Tools Widget
**Objective:** Test bulk operations initiated from UI.

**Steps:**
1. **UI Interaction**: Use MCP Tools widget to batch move
   ```python
   # Navigate and select tool
   mcp__puppeteer__puppeteer_navigate(url="http://localhost:5173")
   mcp__puppeteer__puppeteer_select(selector="select[name='tool']",
                                     value="atomic_batch_move")

   # Fill parameters
   mcp__puppeteer__puppeteer_fill(selector="input[name='source_folder']",
                                   value="INBOX")
   mcp__puppeteer__puppeteer_fill(selector="input[name='dest_folder']",
                                   value="Archive")
   mcp__puppeteer__puppeteer_fill(selector="input[name='uids']",
                                   value="[1001, 1002, 1003]")

   # Execute
   mcp__puppeteer__puppeteer_click(selector="button[data-action='execute-tool']")
   time.sleep(2)
   ```

2. **UI Verification**: Check results display
   ```python
   result = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"tool-result\"]').textContent;
   """)

   assert "success" in result.lower()
   assert "moved 3 messages" in result.lower()
   ```

3. **MCP Client Verification**: Verify via independent MCP call
   ```python
   # Check emails no longer in INBOX
   inbox_emails = await session.call_tool("list_cached_emails", {"folder_name": "INBOX"})
   inbox_uids = [e['uid'] for e in inbox_emails]

   assert 1001 not in inbox_uids
   assert 1002 not in inbox_uids
   assert 1003 not in inbox_uids

   # Check emails now in Archive
   archive_emails = await session.call_tool("list_cached_emails", {"folder_name": "Archive"})
   archive_uids = [e['uid'] for e in archive_emails]

   assert 1001 in archive_uids
   assert 1002 in archive_uids
   assert 1003 in archive_uids
   ```

**Expected Results:**
- ✅ UI batch move succeeds
- ✅ All 3 emails moved correctly
- ✅ MCP client verification confirms moves
- ✅ No partial failures or data inconsistency
- ⏱️ Batch operation time < 2 seconds

### Category 3: Folder Operations Workflows

#### Workflow 3.1: List Folders Cross-Interface Consistency
**Objective:** Verify folder lists are consistent across all interfaces.

**Steps:**
1. **MCP Client**: Get folder list
   ```python
   mcp_folders = await session.call_tool("list_folders")
   mcp_folder_names = sorted([f['name'] for f in mcp_folders])
   ```

2. **HTTP Dashboard API**: Get folder list via REST
   ```bash
   curl http://localhost:9437/api/dashboard/mcp/tools \
     -X POST \
     -H "Content-Type: application/json" \
     -d '{"tool": "list_folders", "arguments": {}}'
   ```

3. **UI Verification**: Check folder list in Email Assistant
   ```python
   mcp__puppeteer__puppeteer_fill(selector="input[name='query']",
                                   value="list all folders")
   mcp__puppeteer__puppeteer_click(selector="button[type='submit']")
   time.sleep(1)

   response = mcp__puppeteer__puppeteer_evaluate(script="""
       return document.querySelector('[data-testid=\"assistant-response\"]').textContent;
   """)

   # Extract folder names from response
   for folder_name in mcp_folder_names:
       assert folder_name in response, f"Folder {folder_name} not found in UI"
   ```

4. **Comparison**: All three sources should match
   ```python
   assert set(mcp_folder_names) == set(rest_folder_names)
   assert set(mcp_folder_names) == set(ui_folder_names)
   ```

**Expected Results:**
- ✅ MCP, REST, and UI all return same folders
- ✅ Folder counts match across interfaces
- ✅ No missing or extra folders in any interface
- ⏱️ All folder list calls < 500ms each

#### Workflow 3.2: Hierarchical Folder Display
**Objective:** Verify hierarchical folder structure preserved across interfaces.

**Steps:**
1. **MCP Client**: Get hierarchical folder list
   ```python
   folders = await session.call_tool("list_folders_hierarchical")

   # Verify hierarchy (e.g., INBOX.Subfolder under INBOX)
   root_folders = [f for f in folders if '.' not in f['name']]
   sub_folders = [f for f in folders if '.' in f['name']]
   ```

2. **UI Verification**: Check folder tree rendering
   ```python
   # Navigate to MCP Tools widget
   mcp__puppeteer__puppeteer_select(selector="select[name='tool']",
                                     value="list_folders_hierarchical")
   mcp__puppeteer__puppeteer_click(selector="button[data-action='execute-tool']")
   time.sleep(1)

   result = mcp__puppeteer__puppeteer_evaluate(script="""
       const resultEl = document.querySelector('[data-testid=\"tool-result\"]');
       return resultEl ? JSON.parse(resultEl.textContent) : null;
   """)

   # Verify hierarchy preserved in JSON result
   ```

**Expected Results:**
- ✅ Hierarchy correctly represented in MCP response
- ✅ UI displays hierarchy properly
- ✅ Parent-child relationships preserved
- ✅ Delimiter (.) handled correctly

### Category 4: Multi-Account Workflows

#### Workflow 4.1: Concurrent Multi-Account Operations
**Objective:** Verify system handles concurrent operations on different accounts.

**Steps:**
1. **Setup**: Two MCP client sessions for different accounts
   ```python
   # Session 1: Account 1
   session1 = await create_mcp_session()
   await session1.call_tool("set_current_account", {"account_id": "account1@example.com"})

   # Session 2: Account 2
   session2 = await create_mcp_session()
   await session2.call_tool("set_current_account", {"account_id": "account2@example.com"})
   ```

2. **Concurrent Operations**: Execute operations simultaneously
   ```python
   import asyncio

   # Concurrent searches
   results = await asyncio.gather(
       session1.call_tool("search_emails", {"folder_name": "INBOX", "query": "from:sender1"}),
       session2.call_tool("search_emails", {"folder_name": "INBOX", "query": "from:sender2"})
   )

   result1, result2 = results

   # Verify no cross-contamination
   assert all('sender1' in email['from'] for email in result1)
   assert all('sender2' in email['from'] for email in result2)
   ```

3. **UI Verification**: Check dashboard handles concurrent updates
   ```python
   # Dashboard should show metrics for both accounts without conflict
   # This requires UI to properly handle multiple SSE connections
   ```

**Expected Results:**
- ✅ Concurrent operations succeed without errors
- ✅ No cross-account data contamination
- ✅ Results are account-specific
- ✅ UI handles multiple SSE connections correctly
- ⏱️ Concurrent operations complete in < 3 seconds

#### Workflow 4.2: Account Switch During Operation
**Objective:** Verify graceful handling of account switch mid-operation.

**Steps:**
1. **MCP Client**: Start long-running search on account 1
   ```python
   await session.call_tool("set_current_account", {"account_id": "account1@example.com"})

   # Start search (may take time)
   search_task = asyncio.create_task(
       session.call_tool("search_emails", {"folder_name": "INBOX", "query": "from:*"})
   )
   ```

2. **MCP Client**: Switch account before search completes
   ```python
   await asyncio.sleep(0.5)  # Let search start
   await session.call_tool("set_current_account", {"account_id": "account2@example.com"})
   ```

3. **Verification**: Search should complete with account 1 data or fail gracefully
   ```python
   try:
       result = await search_task
       # Verify result is from account 1 (operation started with account 1 context)
   except Exception as e:
       # Or gracefully fail with clear error
       assert "account switch" in str(e).lower() or "cancelled" in str(e).lower()
   ```

**Expected Results:**
- ✅ Operation completes with original account context OR
- ✅ Operation fails gracefully with clear error
- ✅ No data corruption
- ✅ System remains stable

### Category 5: Performance & Stress Workflows

#### Workflow 5.1: High-Volume Search Performance
**Objective:** Verify system performance under load.

**Steps:**
1. **MCP Client**: Execute search on large mailbox
   ```python
   import time

   start = time.time()
   result = await session.call_tool("search_emails", {
       "folder_name": "INBOX",
       "query": "from:*",  # Broad query
       "limit": 1000
   })
   elapsed = time.time() - start

   assert elapsed < 5.0, f"Search took {elapsed}s, expected < 5s"
   assert len(result) > 0, "Should return results"
   ```

2. **UI Performance**: Measure UI responsiveness
   ```python
   # Use Puppeteer performance API
   perf = mcp__puppeteer__puppeteer_evaluate(script="""
       const perf = performance.getEntriesByType('navigation')[0];
       return {
           domContentLoaded: perf.domContentLoadedEventEnd - perf.fetchStart,
           loadComplete: perf.loadEventEnd - perf.fetchStart,
           domInteractive: perf.domInteractive - perf.fetchStart
       };
   """)

   assert perf['domInteractive'] < 1000, "DOM interactive < 1s"
   assert perf['loadComplete'] < 3000, "Load complete < 3s"
   ```

**Expected Results:**
- ✅ Search of 1000 emails completes in < 5 seconds
- ✅ UI remains responsive during operation
- ✅ No memory leaks or resource exhaustion
- ⏱️ Performance metrics within acceptable ranges

#### Workflow 5.2: Concurrent Tool Executions
**Objective:** Test system under concurrent load.

**Steps:**
1. **MCP Client**: Execute 10 concurrent tool calls
   ```python
   tasks = []
   for i in range(10):
       task = session.call_tool("list_folders")
       tasks.append(task)

   start = time.time()
   results = await asyncio.gather(*tasks)
   elapsed = time.time() - start

   assert all(len(r) > 0 for r in results), "All calls should succeed"
   assert elapsed < 5.0, f"10 concurrent calls took {elapsed}s"
   ```

2. **Backend Verification**: Check backend handled concurrency
   ```bash
   # Check logs for connection pool usage
   grep "connection pool" logs/rustymail.log | tail -20

   # Should see multiple concurrent connections handled
   ```

**Expected Results:**
- ✅ All 10 concurrent calls succeed
- ✅ No deadlocks or race conditions
- ✅ Connection pool properly managed
- ✅ Total time < 5 seconds (benefit from concurrency)

## Test Orchestration Script

### Complete Workflow Test Runner

Create `scripts/run_workflow_tests.sh`:

```bash
#!/bin/bash
set -e

echo "=== RustyMail Complete Workflow Integration Tests ==="

# Start services
echo "Starting backend server..."
cargo build --release --bin rustymail-server
./target/release/rustymail-server &
BACKEND_PID=$!

sleep 3

echo "Starting frontend..."
cd frontend && npm run dev &
FRONTEND_PID=$!

sleep 5

# Run Python MCP client tests
echo "Running MCP client workflow tests..."
source .venv/bin/activate
python scripts/test_workflow_integration.py

# Run UI workflow tests
echo "Running UI workflow tests..."
python scripts/test_ui_workflows.py

# Cleanup
echo "Cleaning up..."
kill $BACKEND_PID
kill $FRONTEND_PID

echo "=== All workflow tests complete! ==="
```

### Test Results Reporting

Create `scripts/generate_workflow_report.py`:

```python
#!/usr/bin/env python3
"""Generate workflow test report with metrics and screenshots."""

import json
from pathlib import Path
from datetime import datetime

def generate_report(test_results: dict):
    """Generate HTML report from test results."""

    report = {
        "timestamp": datetime.now().isoformat(),
        "summary": {
            "total_workflows": len(test_results),
            "passed": sum(1 for r in test_results.values() if r['passed']),
            "failed": sum(1 for r in test_results.values() if not r['passed']),
        },
        "workflows": test_results
    }

    # Save JSON report
    Path("test-results/workflow-report.json").write_text(json.dumps(report, indent=2))

    # Generate HTML report
    html = generate_html_report(report)
    Path("test-results/workflow-report.html").write_text(html)

    print(f"Report generated: test-results/workflow-report.html")
    print(f"Passed: {report['summary']['passed']}/{report['summary']['total_workflows']}")

def generate_html_report(report: dict) -> str:
    """Generate HTML report."""
    # HTML template with test results, timing metrics, screenshots
    return f"""
    <!DOCTYPE html>
    <html>
    <head>
        <title>RustyMail Workflow Test Report</title>
        <style>
            body {{ font-family: Arial, sans-serif; margin: 20px; }}
            .passed {{ color: green; }}
            .failed {{ color: red; }}
            table {{ border-collapse: collapse; width: 100%; }}
            th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
            th {{ background-color: #4CAF50; color: white; }}
        </style>
    </head>
    <body>
        <h1>RustyMail Workflow Integration Test Report</h1>
        <p>Generated: {report['timestamp']}</p>
        <h2>Summary</h2>
        <p>Total: {report['summary']['total_workflows']}</p>
        <p class="passed">Passed: {report['summary']['passed']}</p>
        <p class="failed">Failed: {report['summary']['failed']}</p>

        <h2>Test Results</h2>
        <table>
            <tr>
                <th>Workflow</th>
                <th>Status</th>
                <th>Duration</th>
                <th>Details</th>
            </tr>
            <!-- Test rows -->
        </table>
    </body>
    </html>
    """

if __name__ == "__main__":
    # Load test results
    results = json.loads(Path("test-results/workflow-results.json").read_text())
    generate_report(results)
```

## Success Criteria

Task 69.6 is complete when:
- [ ] All 5 workflow categories documented
- [ ] Test orchestration scripts created
- [ ] At least 10 end-to-end workflows tested
- [ ] Cross-interface consistency validated
- [ ] Performance benchmarks established
- [ ] Test reporting implemented
- [ ] All tests pass on local environment

## Related Tasks

- Task 69.1: MCP client testing ✅
- Task 69.2: Python MCP client ✅
- Task 69.3: Protocol compliance ✅
- Task 69.4: Puppeteer configuration ✅
- Task 69.5: Dashboard UI tests ✅
- Task 69.7: CI/CD integration (next)
