-- Migration: Add threading headers to emails table for conversation tracking
-- in_reply_to: Message-ID of the parent email in the thread
-- references_header: Space-separated list of Message-IDs in the thread chain

ALTER TABLE emails ADD COLUMN in_reply_to TEXT;
ALTER TABLE emails ADD COLUMN references_header TEXT;

-- Index for efficient thread lookups
CREATE INDEX idx_emails_in_reply_to ON emails(in_reply_to);
