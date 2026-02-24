# Task ID: 88

**Title:** Standardize has_attachments Computation During Sync

**Status:** pending

**Dependencies:** None

**Priority:** medium

**Description:** Ensure has_attachments is computed once during sync and stored consistently in the database

**Details:**

Compute has_attachments during email parsing and store consistently:

```rust
// In email_parser.rs
pub fn parse_email(raw_email: &[u8]) -> Result<ParsedEmail> {
    let parsed = mailparse::parse_mail(raw_email)?;
    let mut has_attachments = false;
    let mut attachment_count = 0;
    
    // Check all MIME parts for attachments
    for part in parsed.subparts.iter() {
        let content_disposition = part.get_content_disposition();
        let content_type = part.get_content_type()?;
        
        // Attachment if:
        // 1. Content-Disposition is "attachment"
        // 2. Content-Disposition has a filename
        // 3. Not inline image (unless it has explicit attachment disposition)
        if content_disposition.disposition == DispositionType::Attachment ||
           content_disposition.params.contains_key("filename") ||
           (content_type.mimetype != "text/plain" && 
            content_type.mimetype != "text/html" &&
            !is_inline_image(&part)) {
            has_attachments = true;
            attachment_count += 1;
        }
    }
    
    // Store in database during sync
    let email = ParsedEmail {
        message_id: extract_message_id(&parsed),
        subject: extract_subject(&parsed),
        has_attachments, // This value is now authoritative
        attachment_count,
        // ... other fields
    };
    
    Ok(email)
}

fn is_inline_image(part: &ParsedMail) -> bool {
    let content_id = part.headers.get_first_value("Content-ID").is_some();
    let content_type = part.get_content_type().ok();
    
    content_id && content_type.map_or(false, |ct| ct.mimetype.starts_with("image/"))
}

// In cache_service.rs - ensure both endpoints read the same field
pub async fn list_cached_emails(account_id: i64, folder: &str) -> Result<Vec<EmailSummary>> {
    sqlx::query_as!(
        EmailSummary,
        "SELECT id, uid, subject, from_addr, date, has_attachments FROM emails 
         WHERE account_id = ? AND folder = ?",
        account_id, folder
    )
    .fetch_all(&pool)
    .await
}

pub async fn get_email_by_uid(account_id: i64, folder: &str, uid: i64) -> Result<Email> {
    sqlx::query_as!(
        Email,
        "SELECT * FROM emails WHERE account_id = ? AND folder = ? AND uid = ?",
        account_id, folder, uid
    )
    .fetch_one(&pool)
    .await
}
```

**Test Strategy:**

1. Unit test attachment detection logic with various MIME structures
2. Test inline images vs actual attachments
3. Verify both list and get endpoints return same has_attachments value
4. Test with emails containing text references to attachments but no actual attachments
5. Test edge cases like empty MIME parts
