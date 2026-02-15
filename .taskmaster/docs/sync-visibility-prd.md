# Email Sync Visibility and Control - PRD

## Problem
The user has no visibility into the email sync process. They can't tell which emails are cached locally vs. on the server, can't see sync progress, and can't trigger a full re-download. The "(20 of 1600 emails)" display means "page 20 of 1600 cached" -- but it says nothing about how many emails actually exist on the IMAP server that haven't been cached yet.

## Goal
Give the user full visibility and control over what's cached, what's syncing, and what still needs to be downloaded.

---

## Task 1: Add cached_count to CachedFolder struct and API response

### Backend (cache.rs)
- Add `pub cached_count: i32` to `CachedFolder` struct
- Modify `get_all_cached_folders_for_account()` SQL query to include a subquery: `(SELECT COUNT(*) FROM emails e WHERE e.folder_id = f.id) AS cached_count`
- Update the row mapping to populate `cached_count`

### Frontend (EmailList.tsx)
- Add `cached_count: number` to `CachedFolderDetail` interface
- Change folder dropdown display from `({detail.total_messages.toLocaleString()})` to `({detail.cached_count} / {detail.total_messages})`

### Verify
- `cargo build && cargo test`
- Check `/api/dashboard/cached-folders` response includes `cached_count`
- Folder dropdown shows "Inbox (350 / 1,600)"

---

## Task 2: Create sync progress migration (010_add_sync_progress.sql)

Create `migrations/010_add_sync_progress.sql`:
```sql
ALTER TABLE sync_state ADD COLUMN emails_synced INTEGER DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN emails_total INTEGER DEFAULT 0;
```

Verify the migration applies cleanly.

---

## Task 3: Update sync binary to write progress during sync

### Backend - sync binary (src/bin/sync.rs)
- At start of `sync_folder()`: Set `sync_status='Syncing'`, `emails_total=len(uids)`, `emails_synced=0`
- After each batch: Update `emails_synced += batch_count`
- At end of `sync_folder()` (in `update_sync_state()`): Reset `emails_synced=0`, `emails_total=0`, `sync_status='Idle'`

### Depends on
- Task 2 (migration must exist first)

---

## Task 4: Update SyncState struct and sync status API to include progress fields

### Backend - cache service (src/dashboard/services/cache.rs)
- Add `emails_synced: i32` and `emails_total: i32` to `SyncState` struct
- Update `get_sync_state()` query to SELECT these new columns

### Backend - API handler (src/dashboard/api/handlers.rs)
- Update `get_sync_status()` response JSON to include `emails_synced` and `emails_total`

### Depends on
- Task 2 (migration must exist first)

---

## Task 5: Add force re-sync CLI flag to sync binary

### Sync binary (src/bin/sync.rs)
- Add `#[arg(long)] force: bool` to `Cli` struct
- Pass `force` through to `sync_folder()`
- If `force`: set `last_uid_synced = 0` (search criteria becomes "ALL" instead of "UID X:*")

### Verify
- `cargo build && cargo test`

---

## Task 6: Add force parameter to sync trigger API endpoint

### API handler (src/dashboard/api/handlers.rs)
- In `trigger_email_sync()`: extract `force` from query params
- If `force`: add `--force` arg to the spawned sync command

### Depends on
- Task 5 (force flag must exist in sync binary)

---

## Task 7: Create useSyncStatus hook for frontend

### New hook: frontend/.../hooks/useSyncStatus.ts
- Polls `/sync/status` every 2s when sync is active
- Returns `{ isSyncing, emailsSynced, emailsTotal, error }`

### Depends on
- Task 4 (sync status API must include progress fields)

---

## Task 8: Create SyncStatusPanel component

### New component: frontend/.../components/SyncStatusPanel.tsx (~150 lines)
- Shows sync progress bar when syncing ("Syncing: 350 / 1,600 emails")
- Buttons: "Sync" (incremental), "Sync All Folders", "Force Re-sync" (with confirmation)
- When idle: shows "X cached / Y total" and last sync time

### Depends on
- Task 7 (useSyncStatus hook)
- Task 6 (force re-sync API)

---

## Task 9: Integrate SyncStatusPanel into EmailList and update folder dropdown

### Update EmailList.tsx
- Import and render `<SyncStatusPanel>` in header
- Remove inline sync button and handleSync logic (moved to panel)
- Update folder dropdown to show `cached_count / total_messages`

### Depends on
- Task 1 (cached_count in API)
- Task 8 (SyncStatusPanel component)

---

## Implementation Order

```
Task 1 (cached count) -- small, independent, immediate value
Task 2 (migration)
Task 3 (sync progress writes) -- depends on 2
Task 4 (sync status API) -- depends on 2
Task 5 (force CLI flag) -- independent
Task 6 (force API) -- depends on 5
Task 7 (useSyncStatus hook) -- depends on 4
Task 8 (SyncStatusPanel) -- depends on 7, 6
Task 9 (integrate into EmailList) -- depends on 1, 8
```
