# Task ID: 79

**Title:** Change select_folder to return MailboxInfo

**Status:** done

**Dependencies:** 78 ✓

**Priority:** high

**Description:** Update AsyncImapOps trait signature in src/imap/session.rs from 'Result<(), ImapError>' to 'Result<MailboxInfo, ImapError>'. Update AsyncImapSessionWrapper::select_folder impl to create MailboxInfo from the async-imap Mailbox response, setting the name field to the folder name.

**Details:**

Modify the IMAP folder selection to return mailbox information instead of discarding it:

1. **Update AsyncImapOps trait in `src/imap/session.rs`**:
```rust
#[async_trait]
pub trait AsyncImapOps: Send + Sync {
    // Change from:
    // async fn select_folder(&mut self, folder: &str) -> Result<(), ImapError>;
    // To:
    async fn select_folder(&mut self, folder: &str) -> Result<MailboxInfo, ImapError>;
    
    // Other methods remain unchanged...
}
```

2. **Update AsyncImapSessionWrapper implementation**:
```rust
#[async_trait]
impl AsyncImapOps for AsyncImapSessionWrapper {
    async fn select_folder(&mut self, folder: &str) -> Result<MailboxInfo, ImapError> {
        let mailbox = self.session
            .select(folder)
            .await
            .map_err(|e| ImapError::SelectError(e.to_string()))?;
        
        // Convert async-imap Mailbox to MailboxInfo
        let mut mailbox_info = MailboxInfo::from(mailbox);
        // Ensure the name field is set to the folder name we selected
        mailbox_info.name = folder.to_string();
        
        Ok(mailbox_info)
    }
}
```

3. **Update ImapClient wrapper in `src/imap/client.rs`**:
```rust
impl ImapClient {
    pub async fn select_folder(&mut self, folder: &str) -> Result<MailboxInfo, ImapError> {
        // Change from returning () to returning MailboxInfo
        self.session.select_folder(folder).await
    }
}
```

4. **No changes needed for existing call sites**:
   - All ~30 existing call sites currently use either `let _ = client.select_folder(folder).await?;` or just `client.select_folder(folder).await?;`
   - These will continue to work, simply discarding the new return value
   - The `?` operator will still propagate errors as before
   - Examples of existing usage that require no changes:
     ```rust
     // In sync operations:
     let _ = imap_client.select_folder(&folder_name).await?;
     
     // In email operations:
     imap_client.select_folder(&folder).await?;
     ```

5. **Benefits of this change**:
   - Future code can access mailbox metadata (exists, recent, unseen, uid_validity, uid_next) when selecting folders
   - No breaking changes to existing code
   - Sets foundation for more intelligent sync operations that can use mailbox metadata

**Test Strategy:**

Verify the select_folder changes work correctly without breaking existing functionality:

1. **Compile and verify no build errors**:
   ```bash
   cargo build
   cargo check
   ```

2. **Run existing tests to ensure no regressions**:
   ```bash
   cargo test
   ```

3. **Create a unit test for the new return value**:
   ```rust
   #[tokio::test]
   async fn test_select_folder_returns_mailbox_info() {
       // Setup mock IMAP session
       let mut client = create_test_imap_client().await;
       
       // Select INBOX
       let mailbox_info = client.select_folder("INBOX").await.unwrap();
       
       // Verify MailboxInfo is populated
       assert_eq!(mailbox_info.name, "INBOX");
       assert!(mailbox_info.exists > 0);
       assert!(mailbox_info.uid_validity.is_some());
       assert!(mailbox_info.uid_next.is_some());
   }
   ```

4. **Test existing call sites still work**:
   - Run the sync binary: `cargo run --bin sync`
   - Verify email operations still function correctly
   - Check that folder selection in various parts of the codebase continues to work

5. **Integration test with real IMAP server**:
   - Connect to a test IMAP account
   - Select various folders and verify MailboxInfo is populated correctly
   - Ensure the folder name in MailboxInfo matches the requested folder

6. **Verify error handling**:
   - Test selecting non-existent folder still returns appropriate error
   - Ensure ImapError::SelectError is properly propagated
