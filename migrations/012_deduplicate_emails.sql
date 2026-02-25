-- Migration: Remove duplicate emails and prevent future duplicates
-- Keeps the row with the highest id (most recently inserted) for each
-- (folder_id, message_id) combination where message_id is not NULL.

-- Step 1: Delete duplicate rows, keeping the newest (highest id) per group
DELETE FROM emails
WHERE message_id IS NOT NULL
  AND id NOT IN (
    SELECT MAX(id)
    FROM emails
    WHERE message_id IS NOT NULL
    GROUP BY folder_id, message_id
  );

-- Note: We do NOT add a UNIQUE constraint on (folder_id, message_id) because
-- the existing ON CONFLICT(folder_id, uid) upsert in sync code can't handle
-- a second unique constraint. The (folder_id, uid) UNIQUE + upsert already
-- prevents duplicates during normal sync. This migration only cleans up
-- historical duplicates that accumulated before the upsert was in place.
