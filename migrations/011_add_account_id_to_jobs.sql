-- Add account_id to background_jobs for per-account filtering
ALTER TABLE background_jobs ADD COLUMN account_id TEXT;

-- Index for filtering jobs by account
CREATE INDEX IF NOT EXISTS idx_background_jobs_account_id ON background_jobs(account_id);
