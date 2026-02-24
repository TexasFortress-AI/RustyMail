# Task ID: 90

**Title:** Fetch and Store In-Reply-To and References Headers

**Status:** pending

**Dependencies:** None

**Priority:** high

**Description:** Extract thread-related headers during sync to enable proper email thread reconstruction

**Details:**

Modify sync to fetch and store threading headers:

```rust
// In email_parser.rs
pub struct ParsedEmail {
    pub message_id: String,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    // ... other fields
}

pub fn parse_email_headers(raw_headers: &[u8]) -> Result<ParsedEmail> {
    let parsed = mailparse::parse_headers(raw_headers)?;
    
    let message_id = parsed.headers.get_first_value("Message-ID")
        .unwrap_or_else(|| generate_synthetic_message_id());
    
    // Extract In-Reply-To header (usually a single message-id)
    let in_reply_to = parsed.headers.get_first_value("In-Reply-To")
        .map(|v| v.trim_matches(|c| c == '<' || c == '>').to_string())
        .filter(|v| !v.is_empty());
    
    // Extract References header (space-separated list of message-ids)
    let references = parsed.headers.get_first_value("References")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    
    Ok(ParsedEmail {
        message_id: message_id.trim_matches(|c| c == '<' || c == '>').to_string(),
        in_reply_to,
        references,
        // ... parse other fields
    })
}

// In imap_sync.rs
pub async fn sync_folder_emails(account_id: i64, folder: &str) -> Result<()> {
    let mut session = connect_imap(account_id).await?;
    session.select(folder)?;
    
    // Fetch with all needed headers including threading headers
    let fetch_items = "UID FLAGS BODY.PEEK[HEADER.FIELDS (From To Cc Subject Date Message-ID In-Reply-To References)]";
    let messages = session.uid_fetch("1:*", fetch_items)?;
    
    for message in messages.iter() {
        let headers = message.header().ok_or(anyhow!("No headers"))?;
        let parsed = parse_email_headers(headers)?;
        
        // Store in database with threading info
        sqlx::query!(
            "INSERT OR REPLACE INTO emails 
             (account_id, folder, message_id, uid, in_reply_to, references_header, ...)
             VALUES (?, ?, ?, ?, ?, ?, ...)",
            account_id, folder, parsed.message_id, message.uid,
            parsed.in_reply_to, parsed.references
        )
        .execute(&pool)
        .await?;
    }
    Ok(())
}

// Enhanced thread traversal
pub async fn get_full_thread(account_id: i64, message_id: &str) -> Result<Vec<Email>> {
    let mut thread_emails = Vec::new();
    let mut visited = HashSet::new();
    let mut to_visit = vec![message_id.to_string()];
    
    while let Some(current_id) = to_visit.pop() {
        if visited.contains(&current_id) {
            continue;
        }
        visited.insert(current_id.clone());
        
        // Get this email
        if let Ok(email) = get_email_by_message_id(account_id, &current_id).await {
            // Add parent to visit list
            if let Some(in_reply_to) = &email.in_reply_to {
                to_visit.push(in_reply_to.clone());
            }
            
            // Add referenced emails to visit list
            if let Some(refs) = &email.references_header {
                for ref_id in refs.split_whitespace() {
                    let cleaned = ref_id.trim_matches(|c| c == '<' || c == '>');
                    to_visit.push(cleaned.to_string());
                }
            }
            
            thread_emails.push(email);
        }
        
        // Find replies to this message
        let replies = sqlx::query_as!(
            Email,
            "SELECT * FROM emails WHERE account_id = ? AND 
             (in_reply_to = ? OR references_header LIKE ?)",
            account_id, current_id, format!("%{}%", current_id)
        )
        .fetch_all(&pool)
        .await?;
        
        for reply in replies {
            to_visit.push(reply.message_id.clone());
        }
    }
    
    thread_emails.sort_by_key(|e| e.date);
    Ok(thread_emails)
}
```

**Test Strategy:**

1. Unit test header extraction with various email formats
2. Test with emails containing multiple references
3. Test thread traversal with known conversation chains
4. Verify circular reference handling
5. Test performance with deep thread hierarchies
