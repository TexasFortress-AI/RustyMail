# Task ID: 6

**Title:** Create email drafter service

**Status:** done

**Dependencies:** 2 ✓

**Priority:** high

**Description:** Implement email_drafter.rs for generating email drafts using configured model

**Details:**

Create src/dashboard/services/ai/email_drafter.rs with EmailDrafter struct, draft_reply() and draft_email() methods. Use model_config to get drafting model settings, call Ollama API to generate text. Include context from original email for replies.

**Test Strategy:**

No test strategy provided.
