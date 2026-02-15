-- Add sync progress tracking columns to sync_state
-- These allow the UI to show real-time sync progress (e.g., "Syncing: 350 / 1600 emails")
ALTER TABLE sync_state ADD COLUMN emails_synced INTEGER DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN emails_total INTEGER DEFAULT 0;
