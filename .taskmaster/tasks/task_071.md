# Task ID: 71

**Title:** Update sync binary to write progress during sync

**Status:** done

**Dependencies:** 70 ✓

**Priority:** high

**Description:** Modify the sync binary to update sync progress in the database as it processes emails, providing real-time visibility into sync operations

**Details:**

In `src/bin/sync.rs`, modify the `sync_folder()` function:

1. At the start of sync:
```rust
// After fetching UIDs from IMAP
let total_emails = uids.len() as i32;
update_sync_progress(&pool, account_id, folder_id, "Syncing", 0, total_emails).await?;
```

2. After each batch is processed:
```rust
// Inside the batch processing loop
let emails_processed = batch_index * batch_size + current_batch.len();
update_sync_progress(&pool, account_id, folder_id, "Syncing", emails_processed as i32, total_emails).await?;
```

3. At the end of sync (in `update_sync_state()`):
```rust
// Reset progress counters when sync completes
sqlx::query!(
    "UPDATE sync_state SET sync_status = 'Idle', emails_synced = 0, emails_total = 0, last_sync = $3 WHERE account_id = $1 AND folder_id = $2",
    account_id, folder_id, Utc::now()
).execute(pool).await?;
```

4. Add helper function:
```rust
async fn update_sync_progress(
    pool: &PgPool,
    account_id: i32,
    folder_id: i32,
    status: &str,
    synced: i32,
    total: i32
) -> Result<()> {
    sqlx::query!(
        "UPDATE sync_state SET sync_status = $1, emails_synced = $2, emails_total = $3 WHERE account_id = $4 AND folder_id = $5",
        status, synced, total, account_id, folder_id
    ).execute(pool).await?;
    Ok(())
}
```

**Test Strategy:**

1. Run sync binary and monitor database updates during sync operation
2. Verify sync_state table shows incremental progress updates
3. Test with large folders (1000+ emails) to ensure progress updates are frequent enough
4. Verify progress resets to 0/0 after sync completes
5. Test error scenarios to ensure partial progress is recorded
6. Monitor performance impact of frequent database updates
