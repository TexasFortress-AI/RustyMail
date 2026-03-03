-- Backfill attachment_parts from attachment_metadata for pre-existing emails.
-- Migration 013 added the column but only new syncs populate it.
-- This one-time UPDATE fills it from attachment_metadata entries.
UPDATE emails
SET attachment_parts = (
    SELECT json_group_array(
        json_object(
            'filename', am.filename,
            'content_type', COALESCE(am.content_type, 'application/octet-stream'),
            'size', am.size_bytes
        )
    )
    FROM attachment_metadata am
    JOIN folders f ON f.id = emails.folder_id
    WHERE am.message_id = emails.message_id
      AND am.account_email = f.account_id
)
WHERE emails.attachment_parts IS NULL
  AND emails.has_attachments = 1
  AND emails.message_id IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM attachment_metadata am2
    JOIN folders f2 ON f2.id = emails.folder_id
    WHERE am2.message_id = emails.message_id
      AND am2.account_email = f2.account_id
  );
