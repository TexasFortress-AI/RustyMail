# Task ID: 20

**Title:** Add MCP server enable/disable controls to Email Assistant chatbot widget

**Status:** done

**Dependencies:** 19 ✓

**Priority:** medium

**Description:** Implement checkboxes/dropdown controls in ChatbotPanel to enable/disable individual MCP servers (low-level and high-level) for the AI assistant to use during conversations.

**Details:**

Based on the current ChatbotPanel.tsx (lines 334-677) and McpTools.tsx components, add MCP server configuration controls to the chatbot: 1) Add a settings dropdown menu next to the debug toggle button in the ChatbotPanel header (around line 364), 2) Create a new state for tracking enabled/disabled MCP servers with localStorage persistence similar to debugMode (line 72-74), 3) Add a collapsible settings panel that shows two sections: 'Low-Level Tools' and 'High-Level AI Tools', 4) For low-level tools, fetch from existing '/dashboard/mcp/tools' endpoint (line 98 in McpTools.tsx), 5) For high-level tools, create new endpoint '/dashboard/mcp/high-level-tools' that calls get_mcp_high_level_tools_jsonrpc_format() from high_level_tools.rs:11, 6) Display each tool as a checkbox with the tool name and description, allowing users to individually enable/disable tools, 7) Pass the enabled tools list to the chatbot query (in the ChatbotQuery interface) so the backend can filter available tools during AI conversations, 8) Use consistent UI patterns from the existing codebase: Radix UI components (checkbox.tsx, collapsible.tsx), similar styling to the debug panel (lines 549-644), and localStorage persistence pattern.

**Test Strategy:**

Test by: 1) Verifying the settings dropdown appears in the chatbot header and is functional, 2) Confirming both low-level and high-level tools are fetched and displayed correctly with checkboxes, 3) Testing that individual tool enable/disable states persist across browser sessions via localStorage, 4) Verifying the enabled tools list is correctly passed to chatbot queries and affects AI responses, 5) Testing the UI responsiveness and proper styling consistency with existing components, 6) Ensuring the new high-level tools endpoint returns the expected tool definitions from the backend.
