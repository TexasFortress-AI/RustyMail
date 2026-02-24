# Task ID: 59

**Title:** Implement Attachment Download Functionality

**Status:** done

**Dependencies:** 55 ✓

**Priority:** low

**Description:** Add capability to download email attachments by implementing secure attachment retrieval with proper access controls and content delivery.

**Details:**

1. Implement download_attachment endpoint:
```rust
async fn download_attachment(
    uid: u64,
    attachment_id: String,
    folder: String,
    account_id: String
) -> Result<AttachmentData, Error> {
    // Verify user has access to account
    // Fetch email from cache or IMAP
    // Extract specific MIME part
    // Return base64 encoded content or file stream
}
```
2. Add attachment caching strategy:
```rust
struct AttachmentCache {
    max_size_mb: u64,
    ttl_seconds: u64,
}
```
3. Implement streaming for large attachments
4. Add virus scanning integration point
5. Support both base64 and direct file download

**Test Strategy:**

1. Test downloading various file types
2. Verify access control prevents unauthorized downloads
3. Test with large attachments (>10MB)
4. Verify content integrity with checksums
5. Test cache hit/miss scenarios
