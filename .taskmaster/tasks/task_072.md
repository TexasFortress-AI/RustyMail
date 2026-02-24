# Task ID: 72

**Title:** Update SyncState struct and sync status API to include progress fields

**Status:** done

**Dependencies:** 70 ✓

**Priority:** high

**Description:** Extend the SyncState data structure and API endpoints to expose sync progress information to the frontend

**Details:**

1. Update `SyncState` struct in `src/dashboard/services/cache.rs`:
```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct SyncState {
    pub account_id: i32,
    pub folder_id: i32,
    pub sync_status: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub last_uid_synced: i64,
    pub emails_synced: i32,  // New field
    pub emails_total: i32,    // New field
}
```

2. Update `get_sync_state()` query:
```rust
let sync_state = sqlx::query_as!(
    SyncState,
    r#"SELECT account_id, folder_id, sync_status, last_sync, last_uid_synced, 
              emails_synced, emails_total 
       FROM sync_state 
       WHERE account_id = $1 AND folder_id = $2"#,
    account_id, folder_id
).fetch_optional(pool).await?;
```

3. Update API response in `src/dashboard/api/handlers.rs` `get_sync_status()`:
```rust
let response = json!({
    "status": sync_state.sync_status,
    "last_sync": sync_state.last_sync,
    "emails_synced": sync_state.emails_synced,
    "emails_total": sync_state.emails_total,
    "is_syncing": sync_state.sync_status == "Syncing"
});
```

**Test Strategy:**

1. Compile and run tests: `cargo build && cargo test`
2. Call `/api/sync/status` endpoint and verify response includes emails_synced and emails_total fields
3. Test during active sync to ensure progress values are returned correctly
4. Test when sync is idle to verify fields return 0
5. Verify backward compatibility if sync_state records don't have these fields yet
