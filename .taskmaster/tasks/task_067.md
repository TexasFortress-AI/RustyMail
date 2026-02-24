# Task ID: 67

**Title:** Change folder dropdown to source folders from cache DB instead of live IMAP

**Status:** done

**Dependencies:** 53 ✓

**Priority:** medium

**Description:** Modify the email folder dropdown in EmailList.tsx to primarily use cached folder names from the database instead of live IMAP queries, ensuring folder visibility even when IMAP authentication fails.

**Details:**

Implement a cache-first approach for the folder dropdown to align with RustyMail's architecture where the cache is the operational source of truth:

1. **Create new backend endpoint GET /api/dashboard/cached-folders**:
   ```rust
   #[get("/api/dashboard/cached-folders")]
   async fn get_cached_folders(
       account_id: web::Query<AccountId>,
       pool: web::Data<SqlitePool>,
   ) -> Result<HttpResponse, Error> {
       let folders = sqlx::query!(
           r#"
           SELECT DISTINCT folder, COUNT(*) as email_count
           FROM emails
           WHERE account_id = ?
           GROUP BY folder
           ORDER BY folder
           "#,
           account_id.0
       )
       .fetch_all(pool.get_ref())
       .await?;
       
       let folder_list: Vec<FolderInfo> = folders
           .into_iter()
           .map(|row| FolderInfo {
               name: row.folder,
               email_count: row.email_count,
           })
           .collect();
       
       Ok(HttpResponse::Ok().json(folder_list))
   }
   ```

2. **Add FolderInfo struct**:
   ```rust
   #[derive(Serialize, Deserialize)]
   struct FolderInfo {
       name: String,
       email_count: i64,
   }
   ```

3. **Register the new endpoint in main.rs**:
   ```rust
   .service(get_cached_folders)
   ```

4. **Update EmailList.tsx to use cached folders as primary source**:
   - Modify the folder fetching logic to first call `/api/dashboard/cached-folders`
   - Keep the existing `/api/dashboard/folders` call as secondary/comparison data
   - Update the state management to handle both cached and live folder lists
   
   ```typescript
   const fetchFolders = async () => {
       try {
           // Primary: Get cached folders
           const cachedResponse = await fetch(`/api/dashboard/cached-folders?account_id=${accountId}`);
           const cachedFolders = await cachedResponse.json();
           setFolders(cachedFolders);
           
           // Secondary: Try to get live folders for comparison
           try {
               const liveResponse = await fetch(`/api/dashboard/folders?account_id=${accountId}`);
               if (liveResponse.ok) {
                   const liveFolders = await liveResponse.json();
                   // Optionally merge or compare with cached folders
                   // Could show sync status indicators
               }
           } catch (liveError) {
               // Live IMAP failed, but we still have cached folders
               console.warn('Live IMAP folder fetch failed, using cached folders only');
           }
       } catch (error) {
           console.error('Failed to fetch cached folders:', error);
       }
   };
   ```

5. **Handle edge cases**:
   - Empty cache scenario (new account with no synced emails yet)
   - Folder names with special characters or hierarchy separators
   - Performance optimization for accounts with many folders
   - Consider adding folder metadata like last sync time

6. **Optional enhancements**:
   - Add visual indicators to show which folders are from cache vs live
   - Show sync status for each folder
   - Add ability to trigger folder sync from the UI

**Test Strategy:**

Verify the cache-first folder implementation with comprehensive testing:

1. **Test cached folder endpoint**:
   - Insert test emails with various folder names into the database
   - Call GET /api/dashboard/cached-folders and verify all distinct folders are returned
   - Verify email counts are accurate for each folder
   - Test with special folder names (spaces, unicode, hierarchy separators)
   - Test performance with 100+ distinct folders

2. **Test frontend integration**:
   - Mock the cached-folders endpoint to return test data
   - Verify folder dropdown populates correctly from cached data
   - Simulate IMAP auth failure and confirm folders still display
   - Test that folder selection still filters emails correctly

3. **Test fallback behavior**:
   - Test with empty cache (no emails in database)
   - Verify graceful handling when cached endpoint fails
   - Test merge logic if implementing live/cached comparison

4. **Integration testing**:
   - Create test account with known folder structure
   - Sync some folders using multi-folder sync (Task 53)
   - Expire OAuth token to simulate auth failure
   - Verify folder dropdown shows all synced folders from cache
   - Verify INBOX is not the only folder shown

5. **Performance testing**:
   - Test query performance with 1M+ emails across 50+ folders
   - Verify no N+1 queries or performance degradation
   - Test concurrent access from multiple sessions
