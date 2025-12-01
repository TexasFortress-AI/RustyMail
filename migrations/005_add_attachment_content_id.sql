-- Migration: Add content_id column to attachment_metadata for inline image support
-- This allows lookup of inline attachments by their Content-ID (used in cid: URIs)

ALTER TABLE attachment_metadata ADD COLUMN content_id TEXT;

-- Index for looking up attachments by Content-ID
CREATE INDEX idx_attachment_content_id ON attachment_metadata(message_id, account_email, content_id);
