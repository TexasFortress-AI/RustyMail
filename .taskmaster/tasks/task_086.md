# Task ID: 86

**Title:** Add Query-Level Deduplication to get_email_thread

**Status:** pending

**Dependencies:** 84

**Priority:** medium

**Description:** Implement DISTINCT query logic in thread retrieval as defense-in-depth against duplicates

**Details:**

Modify the get_thread_emails query to use DISTINCT and proper ordering:

```rust
// In cache_service.rs
pub async fn get_thread_emails(account_id: i64, message_id: &str) -> Result<Vec<Email>> {
    // First, get all emails with this message_id (same email in different folders)
    let mut emails = sqlx::query_as!(
        Email,
        r#"SELECT DISTINCT ON (message_id) 
            id, account_id, folder, message_id, uid, subject, 
            from_addr, to_addr, cc_addr, date, flags, has_attachments,
            in_reply_to, references_header, created_at, updated_at
        FROM emails 
        WHERE account_id = ? AND message_id = ?
        ORDER BY message_id, date ASC, id ASC"#,
        account_id, message_id
    )
    .fetch_all(&pool)
    .await?;
    
    // Then traverse the thread using in_reply_to and references
    let mut thread_message_ids = HashSet::new();
    thread_message_ids.insert(message_id.to_string());
    
    // Find messages this replies to
    if let Some(in_reply_to) = emails.first().and_then(|e| e.in_reply_to.as_ref()) {
        let parent_emails = get_thread_emails(account_id, in_reply_to).await?;
        emails.extend(parent_emails);
    }
    
    // Find messages that reply to this
    let replies = sqlx::query_as!(
        Email,
        r#"SELECT DISTINCT ON (message_id) * FROM emails 
        WHERE account_id = ? 
        AND (in_reply_to = ? OR references_header LIKE ?)
        ORDER BY message_id, date ASC"#,
        account_id, message_id, format!("%{}%", message_id)
    )
    .fetch_all(&pool)
    .await?;
    
    emails.extend(replies);
    
    // Final deduplication and sort
    let mut unique_emails: Vec<Email> = emails.into_iter()
        .collect::<HashMap<String, Email>>()
        .into_values()
        .collect();
    unique_emails.sort_by_key(|e| e.date);
    
    Ok(unique_emails)
}
```

**Test Strategy:**

1. Unit test with intentionally duplicated data
2. Test thread traversal with known conversation chains
3. Verify correct chronological ordering
4. Test performance with large threads
5. Test edge cases like circular references
