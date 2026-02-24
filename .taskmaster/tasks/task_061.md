# Task ID: 61

**Title:** Implement Email Synopsis Generation API

**Status:** done

**Dependencies:** 56 ✓

**Priority:** low

**Description:** Create an API endpoint that generates concise 3-5 line summaries of emails, particularly useful for unread email overviews and quick scanning.

**Details:**

1. Implement get_email_synopsis endpoint:
```rust
async fn get_email_synopsis(
    uid: u64,
    folder: String,
    max_lines: Option<usize>, // Default: 3
    use_ai: Option<bool> // Use AI or simple extraction
) -> Synopsis {
    if use_ai {
        // Integration with LLM API
        let prompt = format!("Summarize in {} lines: {}", max_lines, email_body);
        call_llm_api(prompt).await
    } else {
        // Simple extraction: subject + first N sentences
        extract_summary(email_body, max_lines)
    }
}
```
2. Add bulk synopsis generation for unread emails
3. Implement caching for generated summaries
4. Add configuration for AI provider (OpenAI/Claude/local)
5. Support multiple languages

**Test Strategy:**

1. Test synopsis quality with various email types
2. Verify length constraints are respected
3. Test AI fallback to simple extraction
4. Performance test bulk generation
5. Test with HTML-heavy emails
