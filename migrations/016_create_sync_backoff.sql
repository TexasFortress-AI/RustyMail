-- Per-account retry state so the sync binary backs off from unreachable mail hosts.
CREATE TABLE IF NOT EXISTS sync_backoff (
    account_id TEXT PRIMARY KEY,
    consecutive_failures INTEGER NOT NULL,
    next_attempt_at INTEGER NOT NULL  -- Unix timestamp (seconds)
);
