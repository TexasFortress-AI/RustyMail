# Task ID: 5

**Title:** Wire up browsing tools to high-level variant

**Status:** done

**Dependencies:** 3 ✓

**Priority:** medium

**Description:** Connect existing read-only browsing tools to high-level tool router

**Details:**

Reuse existing handlers for list_accounts, list_folders_hierarchical, list_cached_emails, get_email_by_uid, search_cached_emails, get_folder_stats. Add routing logic in execute_high_level_tool() to call these handlers.

**Test Strategy:**

No test strategy provided.
