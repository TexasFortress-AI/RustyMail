-- Add attachment support to RustyMail
-- Attachments are stored on filesystem, metadata tracked in database

CREATE TABLE attachment_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL,
    account_email TEXT NOT NULL,
    filename TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_type TEXT,
    downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    storage_path TEXT NOT NULL,
    FOREIGN KEY (account_email) REFERENCES accounts(email_address) ON DELETE CASCADE,
    UNIQUE(message_id, account_email, filename)
);

CREATE INDEX idx_attachment_lookup ON attachment_metadata(message_id, account_email);
CREATE INDEX idx_attachment_account ON attachment_metadata(account_email);
CREATE INDEX idx_attachment_downloaded ON attachment_metadata(downloaded_at);
