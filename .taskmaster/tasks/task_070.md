# Task ID: 70

**Title:** Create sync progress migration (010_add_sync_progress.sql)

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Add database columns to track email sync progress in real-time, storing the number of emails synced and total emails to sync

**Details:**

Create migration file `migrations/010_add_sync_progress.sql`:
```sql
-- Add columns to track sync progress
ALTER TABLE sync_state ADD COLUMN emails_synced INTEGER DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN emails_total INTEGER DEFAULT 0;

-- Add index for performance when querying sync state
CREATE INDEX idx_sync_state_account_folder ON sync_state(account_id, folder_id);
```

Ensure migration runner is configured to apply this migration on startup. The columns should default to 0 to maintain backward compatibility with existing records.

**Test Strategy:**

1. Apply migration to test database and verify columns are added successfully
2. Check that existing sync_state records have emails_synced=0 and emails_total=0
3. Verify migration rollback works correctly if needed
4. Test that the application starts successfully with the new schema
5. Verify index creation improves query performance for sync state lookups
