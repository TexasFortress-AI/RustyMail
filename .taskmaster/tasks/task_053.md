# Task ID: 53

**Title:** Enable Multi-Folder Synchronization

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Extend the email synchronization system to support caching of subfolders beyond just INBOX, including critical folders like INBOX/resumes, Sent Items, and INBOX/Contracts.

**Details:**

1. Add folder configuration to sync settings:
```rust
struct SyncConfig {
    folders_to_sync: Vec<String>, // e.g., ["INBOX", "INBOX/resumes", "Sent Items"]
    sync_all_folders: bool,
}
```
2. Modify sync_account function to iterate through configured folders
3. Update database schema to properly handle folder hierarchy
4. Add sync_folder(account_id, folder_name) API endpoint
5. Implement folder sync status tracking
6. Add UI controls for folder selection in admin panel

**Test Strategy:**

1. Test syncing individual subfolders via API
2. Verify folder hierarchy is preserved in cache
3. Test sync_all_folders option syncs entire folder tree
4. Verify get_folder_stats works for all synced folders
5. Performance test with large folder structures (50+ folders)
