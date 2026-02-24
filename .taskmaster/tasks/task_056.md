# Task ID: 56

**Title:** Add Flag-Based Email Filtering

**Status:** done

**Dependencies:** 52 ✓

**Priority:** medium

**Description:** Implement filtering capabilities for emails based on IMAP flags (Seen/Unseen, Flagged, etc.) to support unread email workflows and other flag-based queries.

**Details:**

1. Add flag filter parameters to list_cached_emails:
```rust
pub struct EmailFilter {
    flags_include: Option<Vec<String>>, // Must have these flags
    flags_exclude: Option<Vec<String>>, // Must not have these flags
    unread_only: Option<bool>, // Shorthand for excluding \Seen
}
```
2. Implement list_emails_by_flag endpoint:
```rust
async fn list_emails_by_flag(
    flag: String, // e.g., "Seen", "Flagged", "Answered"
    include: bool, // true = has flag, false = doesn't have flag
    folder: String,
    limit: Option<usize>
) -> Vec<EmailSummary>
```
3. Add database indexes on flags for performance
4. Support standard IMAP flags: \Seen, \Answered, \Flagged, \Deleted, \Draft

**Test Strategy:**

1. Test filtering unread emails (no \Seen flag)
2. Verify multiple flag combinations work correctly
3. Test performance with large datasets
4. Verify flag exclusion logic
5. Test with custom flags if supported by server
