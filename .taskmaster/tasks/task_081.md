# Task ID: 81

**Title:** Fix MailboxInfo construction in all test files

**Status:** done

**Dependencies:** 78 ✓, 79 ✓

**Priority:** high

**Description:** Update all test code that constructs MailboxInfo to include the new uid_validity and uid_next fields. Files include src/imap/client_test.rs and tests/ directory. Also update the mock trait in client_test.rs if its select_folder signature differs from the trait.

**Details:**

1. **Update test constructions in src/imap/client_test.rs**:
   - Search for all instances where MailboxInfo is constructed
   - Add the new fields to each construction:
   ```rust
   MailboxInfo {
       name: "INBOX".to_string(),
       delimiter: Some("/".to_string()),
       flags: vec![],
       exists: 10,
       recent: 2,
       unseen: Some(5),
       uid_validity: Some(12345),  // Add this field
       uid_next: Some(100),        // Add this field
   }
   ```

2. **Update mock trait implementation if needed**:
   - Check if the mock trait in client_test.rs has a select_folder method
   - If it returns MailboxInfo, ensure the mock implementation includes the new fields
   - Example mock update:
   ```rust
   async fn select_folder(&mut self, folder: &str) -> Result<MailboxInfo> {
       Ok(MailboxInfo {
           name: folder.to_string(),
           delimiter: Some("/".to_string()),
           flags: vec!["\\Answered".to_string(), "\\Flagged".to_string()],
           exists: 42,
           recent: 3,
           unseen: Some(7),
           uid_validity: Some(98765),
           uid_next: Some(200),
       })
   }
   ```

3. **Search and update test files in tests/ directory**:
   - Use grep or IDE search: `grep -r "MailboxInfo" tests/`
   - Update any test that constructs MailboxInfo objects
   - Common test patterns to look for:
     - Direct struct construction
     - Builder patterns
     - Test fixtures or helper functions

4. **Update test helper functions**:
   - Look for functions like `create_test_mailbox()` or `mock_mailbox_info()`
   - Add appropriate test values for uid_validity and uid_next
   - Consider using realistic values (e.g., uid_validity: Some(timestamp), uid_next: Some(last_uid + 1))

5. **Fix compilation errors**:
   - After adding the fields, run `cargo test --no-run` to catch any remaining construction sites
   - The compiler will identify all places where MailboxInfo is constructed without the new fields

**Test Strategy:**

1. **Compile all tests without running**:
   ```bash
   cargo test --no-run
   ```
   - Verify no compilation errors related to MailboxInfo construction
   - All struct literal constructions should include uid_validity and uid_next

2. **Run unit tests in imap module**:
   ```bash
   cargo test --package email-assistant --lib imap::
   ```
   - Ensure all imap-related tests pass with the updated MailboxInfo

3. **Run integration tests**:
   ```bash
   cargo test --test '*'
   ```
   - Verify all integration tests that use MailboxInfo still pass

4. **Verify mock behavior**:
   - If mock trait was updated, create a test that calls select_folder on the mock
   - Assert that returned MailboxInfo contains non-None values for uid_validity and uid_next

5. **Grep verification**:
   ```bash
   # Ensure no old-style MailboxInfo constructions remain
   grep -r "MailboxInfo {" src/imap/client_test.rs tests/ | grep -v "uid_validity\|uid_next"
   ```
   - This should return no results if all constructions are updated

6. **Test with different scenarios**:
   - Verify tests handle both Some and None cases for the optional fields
   - Ensure test values are reasonable (e.g., uid_next > 0, uid_validity is a valid timestamp-like value)
