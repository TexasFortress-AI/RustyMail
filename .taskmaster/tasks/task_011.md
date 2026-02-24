# Task ID: 11

**Title:** Add high-level variant support to MCP HTTP backend

**Status:** done

**Dependencies:** 3 ✓, 10 ✓

**Priority:** high

**Description:** Modify mcp_http.rs to support ?variant=high-level query parameter

**Details:**

Update tools/list handler to check for variant parameter and return high-level tools when variant=high-level. Update tools/call handler to route to execute_high_level_tool() for high-level variant. Store variant in session data.

**Test Strategy:**

No test strategy provided.
