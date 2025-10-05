-- Multi-account support migration
-- This migration adds support for multiple email accounts per user

-- Email accounts table
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Account identification
    account_name TEXT NOT NULL UNIQUE, -- User-friendly name (e.g., "Work Gmail", "Personal")
    email_address TEXT NOT NULL UNIQUE,

    -- Provider information
    provider_type TEXT, -- 'gmail', 'outlook', 'yahoo', 'other'
    provider_metadata TEXT, -- JSON object for provider-specific data

    -- IMAP configuration
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL DEFAULT 993,
    imap_user TEXT NOT NULL,
    imap_pass TEXT NOT NULL, -- TODO: Encrypt in production
    imap_use_tls BOOLEAN NOT NULL DEFAULT TRUE,

    -- SMTP configuration (for future sending support)
    smtp_host TEXT,
    smtp_port INTEGER DEFAULT 587,
    smtp_user TEXT,
    smtp_pass TEXT, -- TODO: Encrypt in production
    smtp_use_tls BOOLEAN DEFAULT TRUE,
    smtp_use_starttls BOOLEAN DEFAULT TRUE,

    -- OAuth2 support (for future)
    oauth_provider TEXT, -- 'google', 'microsoft', etc.
    oauth_access_token TEXT,
    oauth_refresh_token TEXT,
    oauth_token_expiry TIMESTAMP,

    -- Account state
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE, -- One account should be default
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

-- Add account_id foreign key to existing folders table
-- First, create a new folders table with the account_id column
CREATE TABLE IF NOT EXISTS folders_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    delimiter TEXT,
    attributes TEXT, -- JSON array of folder attributes
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

-- Migrate existing folders to the new table
-- Only insert folders if there are any to migrate
-- If folders exist, create a temporary default account for backward compatibility
INSERT INTO accounts (
    account_name,
    email_address,
    provider_type,
    imap_host,
    imap_port,
    imap_user,
    imap_pass,
    is_active,
    is_default
)
SELECT
    'Default Account',
    'default@example.com',
    'other',
    'localhost',
    993,
    'default_user',
    'default_pass',
    TRUE,
    TRUE
WHERE EXISTS (SELECT 1 FROM folders LIMIT 1);

-- Migrate existing folders to the new table (only if folders exist)
INSERT INTO folders_new (
    id, account_id, name, delimiter, attributes,
    uidvalidity, uidnext, total_messages, unseen_messages,
    last_sync, created_at, updated_at
)
SELECT
    id,
    1, -- Default account_id
    name,
    delimiter,
    attributes,
    uidvalidity,
    uidnext,
    total_messages,
    unseen_messages,
    last_sync,
    created_at,
    updated_at
FROM folders;

-- Drop old folders table and rename new one
DROP TABLE folders;
ALTER TABLE folders_new RENAME TO folders;

-- Recreate indexes for folders table
CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
CREATE INDEX IF NOT EXISTS idx_folders_name ON folders(account_id, name);

-- Update existing indexes on emails table
-- No schema change needed for emails table, it already references folder_id
-- which now has account_id via the folders table

-- Recreate trigger for folders
CREATE TRIGGER IF NOT EXISTS update_folders_timestamp
    AFTER UPDATE ON folders
    BEGIN
        UPDATE folders SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

-- Add trigger for accounts table
CREATE TRIGGER IF NOT EXISTS update_accounts_timestamp
    AFTER UPDATE ON accounts
    BEGIN
        UPDATE accounts SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

-- Account sessions table (for tracking active connections)
CREATE TABLE IF NOT EXISTS account_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    session_token TEXT NOT NULL UNIQUE,
    connection_state TEXT NOT NULL, -- 'connected', 'disconnected', 'error'
    last_activity TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_account_sessions_account ON account_sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_account_sessions_token ON account_sessions(session_token);

-- Provider auto-configuration templates (optional, can be loaded from code)
-- This table stores common provider configurations for auto-setup
CREATE TABLE IF NOT EXISTS provider_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_type TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    domain_patterns TEXT NOT NULL, -- JSON array of domain patterns (e.g., ["gmail.com", "googlemail.com"])
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

-- Update cache_metadata with new version
INSERT INTO cache_metadata (key, value) VALUES ('schema_version', '2')
ON CONFLICT(key) DO UPDATE SET value = '2', updated_at = CURRENT_TIMESTAMP;
