-- Add attachment_parts column to emails table.
-- Stores a JSON array of attachment metadata objects extracted during sync:
-- [{"filename": "doc.pdf", "content_type": "application/pdf", "size": 12345}, ...]
-- This enables list_email_attachments and get_email_by_uid to return attachment
-- info without requiring a separate download step.
ALTER TABLE emails ADD COLUMN attachment_parts TEXT;
