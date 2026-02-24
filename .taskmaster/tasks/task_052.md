# Task ID: 52

**Title:** Fix Read/Unread Flag Synchronization

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Fix the critical bug where all emails are marked as 'Seen' despite being unread on the server. Ensure proper IMAP flag synchronization using BODY.PEEK[] to avoid marking emails as read during fetch.

**Details:**

1. Modify IMAP fetch commands to use BODY.PEEK[] instead of BODY[] to prevent marking emails as read during sync:
```rust
// In sync module
let fetch_command = format!("{}:{} (FLAGS BODY.PEEK[])", start_uid, end_uid);
```
2. Update flag parsing logic to correctly read \Seen flag from server response
3. Implement proper flag storage in cache database with deduplication
4. Add unit tests to verify flags are preserved during sync
5. Update get_folder_stats to correctly count unread emails based on absence of \Seen flag

**Test Strategy:**

1. Create test IMAP account with known read/unread email counts
2. Run sync and verify unread count matches server state
3. Verify emails remain unread on server after sync
4. Test get_folder_stats returns accurate unread counts
5. Regression test to ensure no duplicate flags in arrays
