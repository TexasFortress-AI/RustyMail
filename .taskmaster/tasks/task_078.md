# Task ID: 78

**Title:** Fix MailboxInfo struct and From<AsyncImapMailbox> conversion

**Status:** done

**Dependencies:** 63 ⏸

**Priority:** high

**Description:** Add uid_validity and uid_next fields to MailboxInfo struct in src/imap/types.rs and fix the broken From<AsyncImapMailbox> conversion to properly extract exists, recent, unseen, uid_validity, uid_next from the async-imap Mailbox struct instead of zeroing them out.

**Details:**

1. **Update MailboxInfo struct** in `src/imap/types.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
    pub exists: u32,
    pub recent: u32,
    pub unseen: Option<u32>,
    pub uid_validity: Option<u32>,  // New field
    pub uid_next: Option<u32>,       // New field
}
```

2. **Fix From<AsyncImapMailbox> implementation**:
```rust
impl From<async_imap::types::Mailbox> for MailboxInfo {
    fn from(mailbox: async_imap::types::Mailbox) -> Self {
        Self {
            name: mailbox.name().to_string(),
            delimiter: mailbox.delimiter().map(|d| d.to_string()),
            flags: mailbox.flags().iter().map(|f| f.to_string()).collect(),
            exists: mailbox.exists,
            recent: mailbox.recent,
            unseen: mailbox.unseen,
            uid_validity: mailbox.uid_validity,
            uid_next: mailbox.uid_next,
        }
    }
}
```

3. **Update From<AsyncImapName> implementation** to include new fields as None:
```rust
impl From<async_imap::types::Name> for MailboxInfo {
    fn from(name: async_imap::types::Name) -> Self {
        Self {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(|d| d.to_string()),
            flags: name.attributes().iter().map(|a| format!("{:?}", a)).collect(),
            exists: 0,
            recent: 0,
            unseen: None,
            uid_validity: None,  // New field
            uid_next: None,      // New field
        }
    }
}
```

4. **Review async-imap version compatibility**:
   - Check that the async-imap version in use (0.8 or planned 0.11+) exposes uid_validity and uid_next fields on the Mailbox struct
   - If fields are not available in current version, consider adding TODO comments for when upgrade happens

5. **Update any code that constructs MailboxInfo manually** to include the new fields

6. **Consider database schema implications**:
   - If MailboxInfo is stored in database, may need migration to add columns
   - Update any SQL queries that insert/select MailboxInfo data

**Test Strategy:**

1. **Compile and verify no build errors**: `cargo build`

2. **Unit test the conversions**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mailbox_conversion_extracts_all_fields() {
        // Create mock AsyncImapMailbox with known values
        // Convert to MailboxInfo
        // Assert all fields including uid_validity and uid_next are properly extracted
    }
    
    #[test]
    fn test_name_conversion_sets_optional_fields_none() {
        // Create mock AsyncImapName
        // Convert to MailboxInfo
        // Assert uid_validity and uid_next are None
    }
}
```

3. **Integration test with real IMAP connection**:
   - Connect to IMAP server and SELECT a mailbox
   - Verify returned MailboxInfo contains valid uid_validity and uid_next values
   - Log the values to confirm they're not zero

4. **Test serialization/deserialization**:
   - Serialize MailboxInfo to JSON and verify new fields are included
   - Deserialize from JSON and verify fields are properly restored

5. **Regression test existing functionality**:
   - Ensure folder listing still works correctly
   - Verify email sync operations continue to function
   - Check that any code depending on MailboxInfo still compiles and runs
