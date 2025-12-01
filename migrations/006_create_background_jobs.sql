-- Background Jobs Table
-- Stores background job state for persistence across server restarts
-- Supports resumable jobs with status tracking

CREATE TABLE IF NOT EXISTS background_jobs (
    job_id TEXT PRIMARY KEY,
    instruction TEXT,  -- The original instruction/description of the job
    status TEXT NOT NULL DEFAULT 'running',  -- 'running', 'completed', 'failed', 'cancelled'
    result_data TEXT,  -- JSON result data when completed
    error_message TEXT,  -- Error message when failed
    started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    -- Fields for resume capability
    resumable BOOLEAN DEFAULT FALSE,  -- Whether this job can be resumed
    resume_checkpoint TEXT,  -- JSON checkpoint data for resuming
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3
);

-- Index for filtering by status
CREATE INDEX IF NOT EXISTS idx_background_jobs_status ON background_jobs(status);

-- Index for cleanup of old completed jobs
CREATE INDEX IF NOT EXISTS idx_background_jobs_completed_at ON background_jobs(completed_at);

-- Trigger to update updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_background_jobs_timestamp
    AFTER UPDATE ON background_jobs
    BEGIN
        UPDATE background_jobs SET updated_at = CURRENT_TIMESTAMP
        WHERE job_id = NEW.job_id;
    END;
