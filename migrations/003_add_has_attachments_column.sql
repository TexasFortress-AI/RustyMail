-- Add has_attachments column to emails table
ALTER TABLE emails ADD COLUMN has_attachments BOOLEAN NOT NULL DEFAULT FALSE;

-- Create index for faster filtering by has_attachments
CREATE INDEX idx_emails_has_attachments ON emails(has_attachments);
