# Task ID: 69

**Title:** Add cached_count to CachedFolder struct and API response

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Enhance the CachedFolder struct to include a count of cached emails and update the API to return this information, enabling the UI to show cached vs total email counts

**Details:**

Backend (cache.rs):
1. Add `pub cached_count: i32` field to the `CachedFolder` struct
2. Modify `get_all_cached_folders_for_account()` function to include a subquery in the SQL:
```rust
let query = r#"
    SELECT f.*, 
           (SELECT COUNT(*) FROM emails e WHERE e.folder_id = f.id) AS cached_count
    FROM folders f
    WHERE f.account_id = $1
"#;
```
3. Update the row mapping to populate the new field:
```rust
cached_count: row.get("cached_count"),
```

Frontend (EmailList.tsx):
1. Add `cached_count: number` to the `CachedFolderDetail` TypeScript interface
2. Update the folder dropdown display logic:
```typescript
// Change from:
({detail.total_messages.toLocaleString()})
// To:
({detail.cached_count.toLocaleString()} / {detail.total_messages.toLocaleString()})
```

**Test Strategy:**

1. Run `cargo build && cargo test` to ensure Rust compilation and tests pass
2. Make API call to `/api/dashboard/cached-folders` and verify response includes `cached_count` field for each folder
3. Verify UI folder dropdown displays format like 'Inbox (350 / 1,600)' showing cached vs total counts
4. Test with folders containing 0 cached emails to ensure proper handling
