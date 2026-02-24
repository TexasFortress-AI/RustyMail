# Task ID: 68

**Title:** Add folder sync status indicators to the folder dropdown

**Status:** done

**Dependencies:** 67 ✓, 53 ✓, 66 ✓

**Priority:** medium

**Description:** Enhance the folder dropdown to display cached email counts, last sync timestamps, and visual distinction between locally cached folders and IMAP-only folders, providing users visibility into folder sync status and available local data.

**Details:**

Extend the folder dropdown implementation from task 67 to include rich metadata about each folder's sync status:

1. **Update backend cached-folders endpoint to include metadata**:
   ```rust
   #[derive(Serialize)]
   struct CachedFolderInfo {
       folder_name: String,
       email_count: i64,
       last_sync_timestamp: Option<DateTime<Utc>>,
       is_cached: bool,
   }
   
   #[get("/api/dashboard/cached-folders")]
   async fn get_cached_folders_with_metadata(
       account_id: web::Query<AccountId>,
       pool: web::Data<SqlitePool>,
   ) -> Result<HttpResponse, Error> {
       // Query cached folders with counts
       let cached_folders = sqlx::query!(
           r#"
           SELECT 
               folder_name,
               COUNT(*) as email_count,
               MAX(received_date) as last_email_date
           FROM emails
           WHERE account_id = ?
           GROUP BY folder_name
           ORDER BY folder_name
           "#,
           account_id.0
       )
       .fetch_all(&pool)
       .await?;
       
       // Get sync timestamps from a sync_status table if available
       let sync_times = get_folder_sync_times(account_id.0, &pool).await?;
       
       let folder_info: Vec<CachedFolderInfo> = cached_folders
           .into_iter()
           .map(|f| CachedFolderInfo {
               folder_name: f.folder_name,
               email_count: f.email_count,
               last_sync_timestamp: sync_times.get(&f.folder_name).cloned(),
               is_cached: true,
           })
           .collect();
       
       Ok(HttpResponse::Ok().json(folder_info))
   }
   ```

2. **Add IMAP-only folder detection when connected**:
   ```rust
   // Extend endpoint to optionally include IMAP folders
   async fn get_all_folders_with_status(
       account_id: i64,
       pool: &SqlitePool,
       imap_session: Option<&ImapSession>,
   ) -> Result<Vec<CachedFolderInfo>, Error> {
       let mut all_folders = get_cached_folders_from_db(account_id, pool).await?;
       
       if let Some(session) = imap_session {
           // Get IMAP folder list
           let imap_folders = session.list(None, "*").await?;
           
           // Find folders that exist in IMAP but not in cache
           for imap_folder in imap_folders {
               if !all_folders.iter().any(|f| f.folder_name == imap_folder.name()) {
                   all_folders.push(CachedFolderInfo {
                       folder_name: imap_folder.name().to_string(),
                       email_count: 0,
                       last_sync_timestamp: None,
                       is_cached: false,
                   });
               }
           }
       }
       
       all_folders
   }
   ```

3. **Update frontend FolderDropdown component**:
   ```tsx
   interface FolderInfo {
     folder_name: string;
     email_count: number;
     last_sync_timestamp?: string;
     is_cached: boolean;
   }
   
   // In FolderDropdown component
   const formatFolderLabel = (folder: FolderInfo): string => {
     let label = folder.folder_name;
     
     if (folder.is_cached && folder.email_count > 0) {
       label += ` (${folder.email_count.toLocaleString()})`;
       
       if (folder.last_sync_timestamp) {
         const syncTime = formatRelativeTime(folder.last_sync_timestamp);
         label += ` • ${syncTime}`;
       }
     }
     
     return label;
   };
   
   const formatRelativeTime = (timestamp: string): string => {
     const date = new Date(timestamp);
     const now = new Date();
     const diffMs = now.getTime() - date.getTime();
     const diffMins = Math.floor(diffMs / 60000);
     
     if (diffMins < 60) return `synced ${diffMins}m ago`;
     if (diffMins < 1440) return `synced ${Math.floor(diffMins / 60)}h ago`;
     return `synced ${Math.floor(diffMins / 1440)}d ago`;
   };
   ```

4. **Add visual styling for folder states**:
   ```tsx
   <SelectItem 
     value={folder.folder_name}
     className={cn(
       "flex items-center justify-between",
       !folder.is_cached && "opacity-60 italic"
     )}
   >
     <span className="flex items-center gap-2">
       {folder.folder_name}
       {folder.is_cached && folder.email_count > 0 && (
         <span className="text-xs text-muted-foreground">
           ({folder.email_count.toLocaleString()})
         </span>
       )}
     </span>
     {folder.is_cached && folder.last_sync_timestamp && (
       <span className="text-xs text-muted-foreground ml-2">
         {formatRelativeTime(folder.last_sync_timestamp)}
       </span>
     )}
     {!folder.is_cached && (
       <span className="text-xs text-muted-foreground ml-2">
         (not synced)
       </span>
     )}
   </SelectItem>
   ```

5. **Add sync status tracking table** (if not already exists):
   ```sql
   CREATE TABLE IF NOT EXISTS folder_sync_status (
       account_id INTEGER NOT NULL,
       folder_name TEXT NOT NULL,
       last_sync_timestamp DATETIME NOT NULL,
       sync_status TEXT DEFAULT 'success',
       PRIMARY KEY (account_id, folder_name),
       FOREIGN KEY (account_id) REFERENCES accounts(id)
   );
   ```

6. **Update sync process to record timestamps**:
   ```rust
   // In sync_folder function
   sqlx::query!(
       "INSERT OR REPLACE INTO folder_sync_status 
        (account_id, folder_name, last_sync_timestamp, sync_status) 
        VALUES (?, ?, ?, ?)",
       account_id,
       folder_name,
       Utc::now(),
       "success"
   )
   .execute(&pool)
   .await?;
   ```

**Test Strategy:**

Verify the enhanced folder dropdown displays accurate sync status information:

1. **Test cached folder metadata display**:
   - Insert test emails into multiple folders with varying counts
   - Verify dropdown shows correct email counts (e.g., "Inbox (2,145)")
   - Test number formatting for large counts (1000+ emails)
   - Verify empty folders show as folder name only without count

2. **Test sync timestamp display**:
   - Manually insert sync timestamps for various folders
   - Verify relative time formatting: "synced 5m ago", "synced 2h ago", "synced 3d ago"
   - Test edge cases: very recent syncs (<1 min), very old syncs (>30 days)
   - Verify folders without sync history don't show timestamp

3. **Test IMAP-only folder detection**:
   - With IMAP connected, verify non-cached folders appear with "(not synced)" indicator
   - Test visual distinction (opacity/italic styling) for unsynced folders
   - Verify cached folders show as normal even when IMAP is connected
   - Test with IMAP disconnected - only cached folders should appear

4. **Test integration with ConnectionStatusIndicator**:
   - When IMAP fails, verify dropdown still shows cached folders with metadata
   - When IMAP reconnects, verify IMAP-only folders appear in dropdown
   - Test switching between accounts maintains correct folder metadata

5. **Performance testing**:
   - Test with 50+ folders to ensure dropdown remains responsive
   - Verify metadata queries don't slow down folder switching
   - Test with folders containing 10,000+ emails

6. **Visual regression testing**:
   - Verify dropdown width accommodates longer labels with metadata
   - Test text truncation for very long folder names with counts
   - Verify proper alignment of counts and timestamps
   - Test dark mode styling for all folder states
