-- RustyMail complete database schema
-- This migration creates all tables for email caching, multi-account support, and sync state

-- Email accounts table
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Account identification
    account_name TEXT NOT NULL UNIQUE,
    email_address TEXT NOT NULL UNIQUE,

    -- Provider information
    provider_type TEXT,
    provider_metadata TEXT,

    -- IMAP configuration
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL DEFAULT 993,
    imap_user TEXT NOT NULL,
    imap_pass TEXT NOT NULL,
    imap_use_tls BOOLEAN NOT NULL DEFAULT TRUE,

    -- SMTP configuration
    smtp_host TEXT,
    smtp_port INTEGER DEFAULT 587,
    smtp_user TEXT,
    smtp_pass TEXT,
    smtp_use_tls BOOLEAN DEFAULT TRUE,
    smtp_use_starttls BOOLEAN DEFAULT TRUE,

    -- OAuth2 support
    oauth_provider TEXT,
    oauth_access_token TEXT,
    oauth_refresh_token TEXT,
    oauth_token_expiry TIMESTAMP,

    -- Account state
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    last_connected TIMESTAMP,
    last_error TEXT,

    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Ensure only one default account exists
CREATE UNIQUE INDEX IF NOT EXISTS idx_single_default_account
    ON accounts(is_default)
    WHERE is_default = TRUE;

-- Folders table to cache IMAP folder structure
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    delimiter TEXT,
    attributes TEXT,
    uidvalidity INTEGER,
    uidnext INTEGER,
    total_messages INTEGER DEFAULT 0,
    unseen_messages INTEGER DEFAULT 0,
    last_sync TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    UNIQUE(account_id, name)
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
    to_addresses TEXT,
    cc_addresses TEXT,
    date TIMESTAMP,
    internal_date TIMESTAMP,
    size INTEGER,
    flags TEXT,
    headers TEXT,
    body_text TEXT,
    body_html TEXT,
    raw_message BLOB,
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
    data BLOB,
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
    sync_status TEXT,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    UNIQUE(folder_id)
);

-- Cache metadata table
CREATE TABLE IF NOT EXISTS cache_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Account sessions table
CREATE TABLE IF NOT EXISTS account_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    session_token TEXT NOT NULL UNIQUE,
    connection_state TEXT NOT NULL,
    last_activity TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- Provider templates for auto-configuration
CREATE TABLE IF NOT EXISTS provider_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_type TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    domain_patterns TEXT NOT NULL,
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL,
    imap_use_tls BOOLEAN NOT NULL,
    smtp_host TEXT NOT NULL,
    smtp_port INTEGER NOT NULL,
    smtp_use_tls BOOLEAN NOT NULL,
    smtp_use_starttls BOOLEAN NOT NULL,
    supports_oauth BOOLEAN NOT NULL DEFAULT FALSE,
    oauth_provider TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
CREATE INDEX IF NOT EXISTS idx_folders_name ON folders(account_id, name);
CREATE INDEX IF NOT EXISTS idx_emails_folder_uid ON emails(folder_id, uid);
CREATE INDEX IF NOT EXISTS idx_emails_message_id ON emails(message_id);
CREATE INDEX IF NOT EXISTS idx_emails_date ON emails(date DESC);
CREATE INDEX IF NOT EXISTS idx_emails_from ON emails(from_address);
CREATE INDEX IF NOT EXISTS idx_emails_subject ON emails(subject);
CREATE INDEX IF NOT EXISTS idx_attachments_email ON attachments(email_id);
CREATE INDEX IF NOT EXISTS idx_sync_state_folder ON sync_state(folder_id);
CREATE INDEX IF NOT EXISTS idx_account_sessions_account ON account_sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_account_sessions_token ON account_sessions(session_token);

-- Triggers to update timestamps
CREATE TRIGGER IF NOT EXISTS update_accounts_timestamp
    AFTER UPDATE ON accounts
    BEGIN
        UPDATE accounts SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

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

-- Insert common provider templates
INSERT INTO provider_templates (
    provider_type, display_name, domain_patterns,
    imap_host, imap_port, imap_use_tls,
    smtp_host, smtp_port, smtp_use_tls, smtp_use_starttls,
    supports_oauth, oauth_provider
) VALUES
    ('gmail', 'Gmail', '["gmail.com", "googlemail.com"]',
     'imap.gmail.com', 993, TRUE,
     'smtp.gmail.com', 587, TRUE, TRUE,
     TRUE, 'google'),
    ('outlook', 'Outlook/Hotmail', '["outlook.com", "hotmail.com", "live.com"]',
     'outlook.office365.com', 993, TRUE,
     'smtp.office365.com', 587, TRUE, TRUE,
     TRUE, 'microsoft'),
    ('yahoo', 'Yahoo Mail', '["yahoo.com", "ymail.com"]',
     'imap.mail.yahoo.com', 993, TRUE,
     'smtp.mail.yahoo.com', 587, TRUE, TRUE,
     FALSE, NULL),
    ('icloud', 'iCloud Mail', '["icloud.com", "me.com", "mac.com"]',
     'imap.mail.me.com', 993, TRUE,
     'smtp.mail.me.com', 587, TRUE, TRUE,
     FALSE, NULL),
    ('fastmail', 'Fastmail', '["fastmail.com", "fastmail.fm"]',
     'imap.fastmail.com', 993, TRUE,
     'smtp.fastmail.com', 587, TRUE, TRUE,
     FALSE, NULL);

-- Initial cache metadata
INSERT INTO cache_metadata (key, value) VALUES
    ('schema_version', '1'),
    ('max_cache_size_mb', '1000'),
    ('max_email_age_days', '30'),
    ('sync_interval_seconds', '300')
ON CONFLICT(key) DO NOTHING;
