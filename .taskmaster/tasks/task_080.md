# Task ID: 80

**Title:** Add update_folder_metadata() to sync.rs

**Status:** done

**Dependencies:** 70 ✓, 79 ✓

**Priority:** high

**Description:** Create a new function in src/bin/sync.rs that takes the MailboxInfo returned by select_folder and writes total_messages, unseen_messages, uidvalidity, uidnext, and last_sync to the folders table. Call this function after select_folder in sync_folder().

**Details:**

Create the `update_folder_metadata()` function in `src/bin/sync.rs`:

```rust
async fn update_folder_metadata(
    pool: &SqlitePool,
    folder_id: i64,
    mailbox_info: &MailboxInfo,
) -> Result<()> {
    let now = Utc::now();
    
    sqlx::query!(
        r#"
        UPDATE folders 
        SET total_messages = ?,
            unseen_messages = ?,
            uidvalidity = ?,
            uidnext = ?,
            last_sync = ?
        WHERE id = ?
        "#,
        mailbox_info.exists as i64,
        mailbox_info.unseen.unwrap_or(0) as i64,
        mailbox_info.uid_validity.unwrap_or(0) as i64,
        mailbox_info.uid_next.unwrap_or(0) as i64,
        now,
        folder_id
    )
    .execute(pool)
    .await?;
    
    Ok(())
}
```

Then modify the `sync_folder()` function to call this after `select_folder()`:

```rust
// In sync_folder() function, after the select_folder call:
let mailbox_info = session.select_folder(&folder.name).await?;

// Add this line to update folder metadata
update_folder_metadata(&pool, folder.id, &mailbox_info).await?;

// Continue with existing sync logic...
```

Key considerations:
- Handle optional fields from MailboxInfo (unseen, uid_validity, uid_next) with sensible defaults
- Use the current timestamp for last_sync
- Ensure the update happens before any errors could occur in the sync process
- The function should be resilient to database errors but propagate them up

**Test Strategy:**

1. Verify the update_folder_metadata function compiles without errors
2. Test with a folder that has messages - check that total_messages, unseen_messages are updated correctly in the database
3. Test with an empty folder - verify it sets total_messages to 0
4. Check that uidvalidity and uidnext are properly stored when present in MailboxInfo
5. Verify last_sync timestamp is updated to current time after each sync
6. Test error handling - simulate database write failure and ensure error propagates correctly
7. Run a full sync and query the folders table to confirm all metadata fields are populated with non-zero values where appropriate
8. Test that subsequent syncs update the metadata correctly, not just the first sync
