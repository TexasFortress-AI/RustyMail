-- Outbox Queue Table
-- Stores emails waiting to be sent, with status tracking and retry logic

CREATE TABLE outbox_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Email identification
    account_email TEXT NOT NULL,
    message_id TEXT,  -- Generated when email is created

    -- Email content (serialized)
    to_addresses TEXT NOT NULL,  -- JSON array
    cc_addresses TEXT,            -- JSON array (optional)
    bcc_addresses TEXT,           -- JSON array (optional)
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    body_html TEXT,
    raw_email_bytes BLOB NOT NULL,  -- Full RFC822 email for IMAP APPEND

    -- Send status tracking
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, sending, sent, failed
    smtp_sent BOOLEAN NOT NULL DEFAULT FALSE,
    outbox_saved BOOLEAN NOT NULL DEFAULT FALSE,
    sent_folder_saved BOOLEAN NOT NULL DEFAULT FALSE,

    -- Retry logic
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    last_error TEXT,

    -- Timestamps
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    smtp_sent_at TIMESTAMP,
    last_retry_at TIMESTAMP,
    completed_at TIMESTAMP,

    FOREIGN KEY (account_email) REFERENCES accounts(email_address) ON DELETE CASCADE
);

CREATE INDEX idx_outbox_queue_status ON outbox_queue(status);
CREATE INDEX idx_outbox_queue_account ON outbox_queue(account_email);
CREATE INDEX idx_outbox_queue_created ON outbox_queue(created_at DESC);
CREATE INDEX idx_outbox_queue_pending ON outbox_queue(status, created_at) WHERE status = 'pending';
