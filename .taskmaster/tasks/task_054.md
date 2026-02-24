# Task ID: 54

**Title:** Fix Attachment Detection and Metadata Parsing

**Status:** done

**Dependencies:** 52 ✓

**Priority:** high

**Description:** Resolve the has_attachments flag inconsistency and implement proper MIME part parsing to accurately detect and store attachment metadata during email synchronization.

**Details:**

1. Implement robust MIME parser using mail-parser crate:
```rust
use mail_parser::{Message, MimeHeaders};

fn parse_attachments(raw_email: &[u8]) -> Vec<AttachmentMetadata> {
    let message = Message::parse(raw_email).unwrap();
    let mut attachments = Vec::new();
    
    for part in message.parts() {
        if part.is_attachment() {
            attachments.push(AttachmentMetadata {
                filename: part.attachment_name().unwrap_or("unnamed").to_string(),
                mime_type: part.content_type().unwrap_or("application/octet-stream"),
                size: part.len(),
                part_id: part.part_id(),
            });
        }
    }
    attachments
}
```
2. Update database schema to store attachment metadata
3. Fix has_attachments flag calculation
4. Ensure consistency across all API endpoints

**Test Strategy:**

1. Test with emails containing various attachment types (PDF, images, documents)
2. Verify has_attachments flag is consistent across list_cached_emails and get_email_by_uid
3. Test inline vs attached content distinction
4. Verify attachment metadata is correctly stored and retrieved
5. Test with multipart/mixed and multipart/alternative messages
