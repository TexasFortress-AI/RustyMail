# Task ID: 84

**Title:** Add Database Deduplication During Email Sync

**Status:** pending

**Dependencies:** None

**Priority:** high

**Description:** Prevent duplicate emails by checking message_id before insertion and updating existing records instead

**Details:**

Modify the sync process to use UPSERT logic based on message_id:

```rust
// In sync_service.rs
pub async fn sync_email(account_id: i64, folder: &str, email: ParsedEmail) -> Result<()> {
    // Check if email already exists
    let existing = sqlx::query!(
        "SELECT id, uid FROM emails WHERE account_id = ? AND folder = ? AND message_id = ?",
        account_id, folder, email.message_id
    )
    .fetch_optional(&pool)
    .await?;
    
    if let Some(existing_email) = existing {
        // Update existing record
        sqlx::query!(
            "UPDATE emails SET 
                flags = ?, 
                has_attachments = ?,
                in_reply_to = ?,
                references_header = ?,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            serde_json::to_string(&email.flags)?,
            email.has_attachments,
            email.in_reply_to,
            email.references,
            existing_email.id
        )
        .execute(&pool)
        .await?;
    } else {
        // Insert new record
        sqlx::query!(
            "INSERT INTO emails (account_id, folder, message_id, uid, subject, from_addr, to_addr, cc_addr, date, flags, has_attachments, in_reply_to, references_header)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            account_id, folder, email.message_id, email.uid, email.subject,
            email.from, email.to, email.cc, email.date,
            serde_json::to_string(&email.flags)?, email.has_attachments,
            email.in_reply_to, email.references
        )
        .execute(&pool)
        .await?;
    }
    Ok(())
}
```

Also add database constraint:
```sql
ALTER TABLE emails ADD CONSTRAINT unique_message_per_folder 
    UNIQUE (account_id, folder, message_id);
```

**Test Strategy:**

1. Unit test sync_email with duplicate message_ids to verify update behavior
2. Test constraint violation handling
3. Create test dataset with known duplicates and verify deduplication
4. Performance test with large batches to ensure UPSERT doesn't slow sync
5. Test edge cases like null message_ids
