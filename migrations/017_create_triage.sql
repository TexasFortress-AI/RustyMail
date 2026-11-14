-- Email triage: what the classifier proposes for each message, and whether
-- `rustymail-triage --apply` has moved it.

-- Highest UID already triaged per account/folder. UIDs are only meaningful
-- together with UIDVALIDITY, so a UIDVALIDITY change restarts the watermark.
CREATE TABLE IF NOT EXISTS triage_watermarks (
    account_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    uidvalidity INTEGER NOT NULL,
    last_uid INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (account_id, folder)
);

CREATE TABLE IF NOT EXISTS triage_proposals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    uidvalidity INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    from_address TEXT NOT NULL,
    subject TEXT NOT NULL,
    bucket TEXT NOT NULL,          -- Spam | Noise | Receipts | Alerts | Needs Reply | Trash | Keep
    confidence REAL,               -- model-reported; NULL for rule matches
    source TEXT NOT NULL,          -- 'rule' | 'model'
    model_id TEXT,                 -- model that produced the bucket, if any
    reason TEXT NOT NULL,
    not_before TIMESTAMP,          -- not to be applied earlier (unexpired code), RFC 3339 UTC
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    applied_at TIMESTAMP,          -- set once the move is verified
    applied_folder TEXT,           -- where apply moved it
    apply_error TEXT,              -- last apply failure, cleared on success
    correction TEXT,               -- reserved: bucket chosen by Chris when the proposal was wrong
    UNIQUE (account_id, folder, uidvalidity, uid)
);

CREATE INDEX IF NOT EXISTS idx_triage_proposals_account_bucket
    ON triage_proposals (account_id, bucket);
