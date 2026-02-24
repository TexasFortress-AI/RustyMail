# Task ID: 57

**Title:** Expose Email Thread Headers and Implement Thread Grouping

**Status:** done

**Dependencies:** 53 ✓

**Priority:** medium

**Description:** Add support for email threading by exposing In-Reply-To and References headers, and implement thread grouping functionality for conversation tracking.

**Details:**

1. Update email parsing to extract thread headers:
```rust
struct EmailHeaders {
    message_id: String,
    in_reply_to: Option<String>,
    references: Option<Vec<String>>,
}
```
2. Implement thread ID calculation algorithm:
```rust
fn calculate_thread_id(headers: &EmailHeaders) -> String {
    // Use oldest message-id in References chain, or current message-id
    if let Some(refs) = &headers.references {
        refs.first().unwrap_or(&headers.message_id).clone()
    } else {
        headers.in_reply_to.as_ref().unwrap_or(&headers.message_id).clone()
    }
}
```
3. Add get_email_thread endpoint to retrieve full conversation
4. Update database schema to store thread relationships
5. Implement thread-aware search

**Test Strategy:**

1. Test with known email threads to verify correct grouping
2. Test thread calculation with various header combinations
3. Verify orphaned emails get their own thread ID
4. Test performance of thread queries
5. Verify thread ordering is chronological
