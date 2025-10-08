-- RustyMail Email Cache Database Schema
-- Using email addresses as natural primary keys

-- Accounts table with email_address as primary key
CREATE TABLE IF NOT EXISTS accounts (
    email_address TEXT PRIMARY KEY NOT NULL,
    display_name TEXT,
    imap_server TEXT,
    imap_port INTEGER,
    smtp_server TEXT,
    smtp_port INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Folders table - references account by email_address
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_email TEXT NOT NULL,
    name TEXT NOT NULL,
    uidvalidity INTEGER,
    uidnext INTEGER,
    unseen INTEGER,
    recent INTEGER,
    total_messages INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_email) REFERENCES accounts(email_address) ON DELETE CASCADE,
    UNIQUE(account_email, name)
);

-- Emails table
CREATE TABLE IF NOT EXISTS emails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    from_address TEXT,
    from_name TEXT,
    to_addresses TEXT NOT NULL,
    cc_addresses TEXT NOT NULL,
    date DATETIME,
    internal_date DATETIME,
    size INTEGER,
    flags TEXT NOT NULL,
    body_text TEXT,
    body_html TEXT,
    cached_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    UNIQUE(folder_id, uid)
);

-- Sync state table
CREATE TABLE IF NOT EXISTS sync_state (
    folder_id INTEGER PRIMARY KEY,
    last_uid_synced INTEGER,
    last_full_sync DATETIME,
    last_incremental_sync DATETIME,
    sync_status TEXT DEFAULT 'idle',
    error_message TEXT,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder_id);
CREATE INDEX IF NOT EXISTS idx_emails_uid ON emails(uid);
CREATE INDEX IF NOT EXISTS idx_emails_date ON emails(date DESC);
CREATE INDEX IF NOT EXISTS idx_emails_subject ON emails(subject);
CREATE INDEX IF NOT EXISTS idx_emails_from ON emails(from_address);
CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_email);
