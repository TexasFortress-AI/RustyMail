# Task ID: 19

**Title:** Add tabs to MCP Email Tools widget in web UI - one tab for low-level MCP tools, another tab for high-level AI-powered MCP tools

**Status:** done

**Dependencies:** 3 ✓, 5 ✓

**Priority:** medium

**Description:** Enhance the existing McpTools component to display two separate tabs: one showing all the existing low-level MCP tools and another showing the high-level AI-powered MCP tools

**Details:**

Modify the existing McpTools.tsx component (src/dashboard/components/McpTools.tsx) to use Radix UI Tabs component from components/ui/tabs.tsx. The component should: 1) Import and use Tabs, TabsList, TabsTrigger, and TabsContent from '../ui/tabs', 2) Create two tab triggers: 'Low-Level Tools' and 'AI Tools', 3) Move existing tool fetching and display logic into the 'Low-Level Tools' tab content, 4) Add a new API endpoint fetch to get high-level tools from the backend endpoint '/dashboard/mcp/high-level-tools' (which needs to be implemented to call get_mcp_high_level_tools_jsonrpc_format() from high_level_tools.rs), 5) Display the high-level tools in the 'AI Tools' tab with the same UI pattern as existing tools, 6) Maintain all existing functionality including parameter auto-filling, execution, and result display for both tool types, 7) Update the header to show total tools from both tabs, 8) Ensure proper state management so expanding/collapsing tools, parameters, and results work independently between tabs. The backend route handler should call execute_high_level_tool() for AI tool executions and existing execute_mcp_tool_inner() for low-level tools.

**Test Strategy:**

Test by: 1) Verifying both tabs are visible and clickable in the MCP Tools widget, 2) Confirming the 'Low-Level Tools' tab shows existing tools with unchanged functionality, 3) Verifying the 'AI Tools' tab displays the 12 high-level tools (process_email_instructions, draft_reply, draft_email, list_accounts, etc.), 4) Testing parameter auto-filling works in both tabs based on current email context, 5) Testing tool execution works correctly for both low-level and high-level tools with proper API routing, 6) Verifying results display properly in both tabs, 7) Testing tab switching preserves expanded tool states and parameter values, 8) Confirming the total tool count in header updates correctly when switching tabs
