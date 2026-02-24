# Task ID: 13

**Title:** Test high-level MCP variant with Claude Desktop

**Status:** deferred

**Dependencies:** 12 ✓

**Priority:** medium

**Description:** Integration testing of complete high-level tool flow

**Details:**

Configure Claude Desktop to use rustymail-mcp-stdio-high-level binary. Migration 004 (ai_model_configurations table) has been applied with default models: qwen2.5:7b for tool_calling and llama3.3:70b for drafting. Test workflow: 1) Use set_tool_calling_model and set_drafting_model tools to configure actual models instead of defaults, 2) Test browsing tools (list_accounts, list_folders_hierarchical, get_email_by_uid), 3) Test configuration tools (get_model_configurations), 4) Test drafting tools (draft_reply, draft_email), 5) Test process_email_instructions with simple workflows. Verify tool count is ~12 instead of 26+. Process_email_instructions tool should now work properly after database migration fix.

**Test Strategy:**

Configure Claude Desktop with rustymail-mcp-stdio-high-level, verify 12 tools available instead of 26+. First configure models using configuration tools, then test each tool category: browsing (list accounts/folders/emails), drafting (generate replies/emails), and workflow execution (process_email_instructions). Confirm all tools work without database errors.

## Subtasks

### 13.1. Configure Claude Desktop with high-level MCP binary

**Status:** pending  
**Dependencies:** None  

Set up Claude Desktop to use rustymail-mcp-stdio-high-level binary and verify connection

**Details:**

Update Claude Desktop configuration to point to target/release/rustymail-mcp-stdio-high-level binary. Ensure server is running on configured port. Verify Claude Desktop shows ~12 tools available instead of 26+.

### 13.2. Configure AI models using configuration tools

**Status:** pending  
**Dependencies:** 13.1  

Use set_tool_calling_model and set_drafting_model to replace default configurations

**Details:**

Migration 004 created default configurations (qwen2.5:7b for tool_calling, llama3.3:70b for drafting). Use the MCP configuration tools to set actual models the user wants to use. Test get_model_configurations to verify settings are saved correctly.

### 13.3. Test browsing tools functionality

**Status:** pending  
**Dependencies:** 13.2  

Test list_accounts, list_folders_hierarchical, list_cached_emails, and get_email_by_uid tools

**Details:**

Verify all browsing tools work correctly with high-level MCP variant. Test that these tools provide the same functionality as the low-level variant but through the simplified interface.

### 13.4. Test drafting tools functionality

**Status:** pending  
**Dependencies:** 13.2  

Test draft_reply and draft_email tools using configured drafting model

**Details:**

Verify AI-powered drafting tools work with the configured drafting model from step 2. Test that drafts are generated with appropriate quality and relevance to input context.

### 13.5. Test process_email_instructions workflow execution

**Status:** pending  
**Dependencies:** 13.2, 13.3  

Test the main workflow tool with simple email management instructions

**Details:**

Test process_email_instructions with simple workflows like 'list unread emails in INBOX' or 'show folder statistics'. Should now work correctly after ai_model_configurations table migration fix. Verify the tool uses other available tools to complete the workflow.
