# Task ID: 55

**Title:** Implement Attachment Metadata API Endpoints

**Status:** done

**Dependencies:** 54 ✓

**Priority:** medium

**Description:** Create new API endpoints to expose attachment metadata including filenames, MIME types, sizes, and enable filtering emails by attachment type.

**Details:**

1. Implement get_attachment_metadata endpoint:
```rust
#[derive(Serialize)]
struct AttachmentInfo {
    attachment_id: String,
    filename: String,
    mime_type: String,
    size: u64,
}

async fn get_attachment_metadata(uid: u64, folder: String) -> Vec<AttachmentInfo>
```
2. Implement search_emails_by_attachment endpoint with MIME type filtering:
```rust
async fn search_emails_by_attachment(
    mime_types: Vec<String>, // e.g., ["image/*", "application/pdf"]
    folder: String,
    limit: Option<usize>
) -> Vec<EmailSummary>
```
3. Add attachment count and total size to email responses
4. Implement wildcard MIME type matching (e.g., image/*)

**Test Strategy:**

1. Test get_attachment_metadata returns correct data for emails with multiple attachments
2. Verify MIME type filtering works with wildcards
3. Test performance with large attachment queries
4. Verify empty results for emails without attachments
5. Test edge cases like emails with 50+ attachments
