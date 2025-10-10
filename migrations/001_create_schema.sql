-- RustyMail Database Schema
-- Uses email_address as the primary key for accounts (KISS principle)
-- NOTE: sqlx automatically wraps migrations in transactions, so we don't need explicit BEGIN/COMMIT

-- Accounts table - email_address is the primary key
CREATE TABLE accounts (
    -- Account identification - email is the primary key
    email_address TEXT PRIMARY KEY,
    display_name TEXT NOT NULL UNIQUE,

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

CREATE UNIQUE INDEX idx_single_default_account
    ON accounts(is_default)
    WHERE is_default = TRUE;

CREATE TRIGGER update_accounts_timestamp
    AFTER UPDATE ON accounts
    BEGIN
        UPDATE accounts SET updated_at = CURRENT_TIMESTAMP
        WHERE email_address = NEW.email_address;
    END;

-- Folders table - references account by email_address
CREATE TABLE folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL,  -- Email address of the account
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
    FOREIGN KEY (account_id) REFERENCES accounts(email_address) ON DELETE CASCADE,
    UNIQUE(account_id, name)
);

CREATE INDEX idx_folders_account ON folders(account_id);
CREATE INDEX idx_folders_name ON folders(account_id, name);

CREATE TRIGGER update_folders_timestamp
    AFTER UPDATE ON folders
    BEGIN
        UPDATE folders SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

-- Emails table
CREATE TABLE emails (
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

CREATE INDEX idx_emails_folder_uid ON emails(folder_id, uid);
CREATE INDEX idx_emails_message_id ON emails(message_id);
CREATE INDEX idx_emails_date ON emails(date DESC);
CREATE INDEX idx_emails_from ON emails(from_address);
CREATE INDEX idx_emails_subject ON emails(subject);

CREATE TRIGGER update_emails_timestamp
    AFTER UPDATE ON emails
    BEGIN
        UPDATE emails SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

-- Sync state table - tracks email synchronization progress
CREATE TABLE sync_state (
    folder_id INTEGER PRIMARY KEY,
    last_uid_synced INTEGER,
    last_full_sync TIMESTAMP,
    last_incremental_sync TIMESTAMP,
    sync_status TEXT NOT NULL DEFAULT 'Idle',
    error_message TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_state_folder ON sync_state(folder_id);
CREATE INDEX idx_sync_state_status ON sync_state(sync_status);

CREATE TRIGGER update_sync_state_timestamp
    AFTER UPDATE ON sync_state
    BEGIN
        UPDATE sync_state SET updated_at = CURRENT_TIMESTAMP WHERE folder_id = NEW.folder_id;
    END;
