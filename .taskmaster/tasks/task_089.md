# Task ID: 89

**Title:** Fix IMAP BODY.PEEK Usage to Preserve Unread Status

**Status:** pending

**Dependencies:** None

**Priority:** medium

**Description:** Audit and fix all IMAP fetch operations to use BODY.PEEK variants to avoid marking emails as read

**Details:**

Replace all BODY[] fetches with BODY.PEEK[] to preserve read status:

```rust
// In imap_client.rs
pub async fn fetch_email_content(session: &mut Session, uid: u32) -> Result<Vec<u8>> {
    // WRONG: This marks the email as read
    // let messages = session.uid_fetch(uid.to_string(), "BODY[]")?;
    
    // CORRECT: Use PEEK to avoid changing flags
    let messages = session.uid_fetch(uid.to_string(), "BODY.PEEK[]")?;
    
    if let Some(message) = messages.iter().next() {
        if let Some(body) = message.body() {
            return Ok(body.to_vec());
        }
    }
    Err(anyhow!("Failed to fetch email body"))
}

pub async fn fetch_headers(session: &mut Session, uid_range: &str) -> Result<Vec<EmailHeader>> {
    // Fetch headers without marking as read
    let fetch_items = "UID FLAGS BODY.PEEK[HEADER.FIELDS (From To Cc Subject Date Message-ID In-Reply-To References)]";
    let messages = session.uid_fetch(uid_range, fetch_items)?;
    
    let mut headers = Vec::new();
    for message in messages.iter() {
        headers.push(parse_header_fields(message)?);
    }
    Ok(headers)
}

// One-time flag resync function
pub async fn resync_folder_flags(account_id: i64, folder: &str) -> Result<()> {
    let mut session = connect_imap(account_id).await?;
    session.select(folder)?;
    
    // Get all UIDs in cache
    let cached_uids = get_cached_uids(account_id, folder).await?;
    
    // Fetch current flags from server
    let fetch_items = "UID FLAGS";
    let messages = session.uid_fetch(format!("{}:*", cached_uids.first().unwrap_or(&1)), fetch_items)?;
    
    let mut tx = pool.begin().await?;
    for message in messages.iter() {
        let uid = message.uid.ok_or(anyhow!("No UID"))?;
        let flags = message.flags();
        
        // Update flags in cache
        sqlx::query!(
            "UPDATE emails SET flags = ? WHERE account_id = ? AND folder = ? AND uid = ?",
            serde_json::to_string(&flags)?, account_id, folder, uid as i64
        )
        .execute(&mut tx)
        .await?;
    }
    tx.commit().await?;
    
    info!("Resynced {} email flags for folder {}", messages.len(), folder);
    Ok(())
}
```

**Test Strategy:**

1. Audit all IMAP fetch calls to ensure BODY.PEEK usage
2. Test sync operation doesn't change server-side \Seen flags
3. Manually verify with IMAP client that emails remain unread after sync
4. Test flag resync function with a test folder
5. Verify performance impact of PEEK operations
