# Task ID: 10

**Title:** Implement process_email_instructions tool

**Status:** done

**Dependencies:** 9 ✓

**Priority:** high

**Description:** Create MCP tool handler for complex email workflow execution

**Details:**

Implement handler that takes natural language instruction, gets tool-calling model config, converts all low-level MCP tools to Ollama format, calls AgentExecutor, formats result. Include logic to detect when user feedback is needed and return questions in JSON format.

**Test Strategy:**

No test strategy provided.
