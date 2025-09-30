-- Initial schema for RustyMail email caching
-- This migration creates tables for caching emails, folders, and sync state

-- Folders table to cache IMAP folder structure
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    delimiter TEXT,
    attributes TEXT, -- JSON array of folder attributes
    uidvalidity INTEGER,
    uidnext INTEGER,
    total_messages INTEGER DEFAULT 0,
    unseen_messages INTEGER DEFAULT 0,
    last_sync TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Emails table to cache email metadata and content
CREATE TABLE IF NOT EXISTS emails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    from_address TEXT,
    from_name TEXT,
    to_addresses TEXT, -- JSON array
    cc_addresses TEXT, -- JSON array
    date TIMESTAMP,
    internal_date TIMESTAMP,
    size INTEGER,
    flags TEXT, -- JSON array of flags
    headers TEXT, -- JSON object with all headers
    body_text TEXT,
    body_html TEXT,
    raw_message BLOB, -- Complete raw email for MIME parsing
    cached_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    UNIQUE(folder_id, uid)
);

-- Attachments table for email attachments
CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email_id INTEGER NOT NULL,
    filename TEXT,
    content_type TEXT,
    content_id TEXT,
    size INTEGER,
    data BLOB, -- Attachment content (lazy loaded)
    cached BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (email_id) REFERENCES emails(id) ON DELETE CASCADE
);

-- Sync state table to track synchronization progress
CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER NOT NULL,
    last_uid_synced INTEGER,
    last_full_sync TIMESTAMP,
    last_incremental_sync TIMESTAMP,
    sync_status TEXT, -- 'idle', 'syncing', 'error'
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    UNIQUE(folder_id)
);

-- Cache metadata table for managing cache size and policies
CREATE TABLE IF NOT EXISTS cache_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_emails_folder_uid ON emails(folder_id, uid);
CREATE INDEX IF NOT EXISTS idx_emails_message_id ON emails(message_id);
CREATE INDEX IF NOT EXISTS idx_emails_date ON emails(date DESC);
CREATE INDEX IF NOT EXISTS idx_emails_from ON emails(from_address);
CREATE INDEX IF NOT EXISTS idx_emails_subject ON emails(subject);
CREATE INDEX IF NOT EXISTS idx_attachments_email ON attachments(email_id);
CREATE INDEX IF NOT EXISTS idx_sync_state_folder ON sync_state(folder_id);

-- Triggers to update timestamps
CREATE TRIGGER IF NOT EXISTS update_folders_timestamp
    AFTER UPDATE ON folders
    BEGIN
        UPDATE folders SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_emails_timestamp
    AFTER UPDATE ON emails
    BEGIN
        UPDATE emails SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_sync_state_timestamp
    AFTER UPDATE ON sync_state
    BEGIN
        UPDATE sync_state SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

-- Initial cache metadata
INSERT INTO cache_metadata (key, value) VALUES
    ('version', '1.0.0'),
    ('max_cache_size_mb', '1000'),
    ('max_email_age_days', '30'),
    ('sync_interval_seconds', '300')
ON CONFLICT(key) DO NOTHING;