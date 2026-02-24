# Task ID: 77

**Title:** Integrate SyncStatusPanel into EmailList and update folder dropdown

**Status:** done

**Dependencies:** 69 ✓, 76 ✓

**Priority:** low

**Description:** Integrate the new SyncStatusPanel component into the EmailList view and update the folder dropdown to display cached vs total email counts

**Details:**

Update `frontend/src/components/EmailList.tsx`:

```typescript
// Add imports
import { SyncStatusPanel } from './SyncStatusPanel';

// Remove existing sync-related state and handlers
// Remove: const [isSyncing, setIsSyncing] = useState(false);
// Remove: handleSync function

// In the component JSX, add SyncStatusPanel after the header
export function EmailList({ accountId }: EmailListProps) {
  // ... existing code ...
  
  return (
    <div className="flex flex-col h-full">
      {/* Existing header */}
      <div className="border-b px-4 py-3">
        {/* ... existing header content ... */}
      </div>
      
      {/* Add SyncStatusPanel */}
      <SyncStatusPanel 
        accountId={accountId} 
        folderId={selectedFolder?.id}
        onSyncComplete={() => {
          // Refetch folders and emails after sync
          refetchFolders();
          refetchEmails();
        }}
      />
      
      {/* Update folder dropdown display */}
      <Select value={selectedFolder?.id} onValueChange={handleFolderChange}>
        <SelectTrigger>
          <SelectValue>
            {selectedFolder && folderDetails ? (
              <span>
                {selectedFolder.name} 
                <span className="text-gray-500 ml-1">
                  ({folderDetails.cached_count.toLocaleString()} / {folderDetails.total_messages.toLocaleString()})
                </span>
              </span>
            ) : (
              'Select folder'
            )}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {folders?.map((folder) => {
            const detail = folderDetails?.find(d => d.folder_id === folder.id);
            return (
              <SelectItem key={folder.id} value={folder.id}>
                {folder.name}
                {detail && (
                  <span className="text-gray-500 ml-1">
                    ({detail.cached_count.toLocaleString()} / {detail.total_messages.toLocaleString()})
                  </span>
                )}
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
      
      {/* Rest of the component */}
    </div>
  );
}
```

Remove the inline sync button and related logic since it's now handled by SyncStatusPanel.

**Test Strategy:**

1. Verify SyncStatusPanel renders in the EmailList header area
2. Test folder dropdown shows format 'Inbox (350 / 1,600)' for each folder
3. Verify sync operations triggered from panel update the email list
4. Test that removing old sync button doesn't break any functionality
5. Verify folder counts update after sync completes
6. Test responsive layout with SyncStatusPanel at different screen sizes
7. Ensure no duplicate sync triggers or race conditions
8. Test that folder selection still works correctly with new display format
