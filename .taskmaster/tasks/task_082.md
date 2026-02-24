# Task ID: 82

**Title:** Build, test, and verify folder metadata functionality

**Status:** done

**Dependencies:** 68 ✓, 79 ✓, 80 ✓, 81 ✓

**Priority:** high

**Description:** Execute cargo build and cargo test to ensure all code compiles and tests pass, then rebuild and restart services via PM2. Verify the implementation by triggering a sync operation and confirming that the folders table contains accurate metadata including non-zero total_messages and populated uidvalidity/uidnext values, and that the frontend displays correct cached/total counts in the folder dropdown.

**Details:**

This task involves building, testing, and verifying the complete folder metadata functionality implemented in previous tasks:

1. **Build and test the Rust codebase**:
   ```bash
   # Clean build to ensure all changes are compiled
   cargo clean
   cargo build --release
   
   # Run all tests to verify no regressions
   cargo test
   ```

2. **Rebuild and restart services via PM2**:
   ```bash
   # Stop existing services
   pm2 stop all
   
   # Rebuild the sync service binary
   cargo build --release --bin sync
   
   # Rebuild the dashboard service
   cargo build --release --bin dashboard
   
   # Restart services with PM2
   pm2 start ecosystem.config.js
   pm2 logs --lines 50  # Check for startup errors
   ```

3. **Trigger a sync operation**:
   - Use the admin panel or API to trigger a full account sync
   - Monitor sync logs to ensure folders are being selected and metadata is being updated
   - Example API call:
   ```bash
   curl -X POST http://localhost:8080/api/sync/trigger?account_id=1
   ```

4. **Verify database folder metadata**:
   ```sql
   -- Check folders table has been populated with metadata
   SELECT id, folder_name, total_messages, unseen_messages, 
          uidvalidity, uidnext, last_sync 
   FROM folders 
   WHERE account_id = 1;
   
   -- Verify non-zero message counts for folders with emails
   SELECT folder_name, total_messages 
   FROM folders 
   WHERE total_messages > 0;
   
   -- Check UIDVALIDITY and UIDNEXT are populated
   SELECT folder_name, uidvalidity, uidnext 
   FROM folders 
   WHERE uidvalidity IS NOT NULL AND uidnext IS NOT NULL;
   ```

5. **Verify frontend folder dropdown displays accurate counts**:
   - Navigate to the email dashboard
   - Open the folder dropdown
   - Verify each folder shows:
     - Correct cached email count (matching database)
     - Total message count from IMAP metadata
     - Format: "Folder Name (cached/total)"
   - Check that recently synced folders show updated counts
   - Verify visual distinction between cached and non-cached folders

**Test Strategy:**

1. **Build verification**:
   - Confirm `cargo build --release` completes without errors
   - Verify both sync and dashboard binaries are created in target/release/

2. **Test suite verification**:
   - Run `cargo test` and ensure all tests pass
   - Pay special attention to tests related to:
     - select_folder returning MailboxInfo
     - update_folder_metadata function
     - Folder metadata endpoints

3. **Service restart verification**:
   - Check PM2 status shows all services running: `pm2 status`
   - Monitor logs for any startup errors: `pm2 logs --lines 100`
   - Verify services are responding: `curl http://localhost:8080/health`

4. **Sync operation verification**:
   - Trigger sync and monitor logs for folder selection messages
   - Verify log entries show "Updating folder metadata for [folder_name]"
   - Check sync completes without errors

5. **Database verification**:
   - Query folders table and verify:
     - All synced folders have non-zero total_messages (where applicable)
     - uidvalidity is a positive integer for all folders
     - uidnext is populated and greater than 0
     - last_sync timestamp is recent (within last few minutes)
   - Compare total_messages in DB with actual IMAP folder counts

6. **Frontend verification**:
   - Load email dashboard and check folder dropdown
   - Verify format shows "Inbox (1,234/1,500)" style counts
   - Select different folders and verify counts update
   - Check that cached folders show different styling than non-cached
   - Test with folders containing 0 emails to verify "0/0" display
   - Verify number formatting for large counts (e.g., "10.2K/15.3K")
