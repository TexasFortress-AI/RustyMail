# Task ID: 76

**Title:** Create SyncStatusPanel component

**Status:** done

**Dependencies:** 75 ✓, 74 ✓

**Priority:** low

**Description:** Build a comprehensive UI component that displays sync progress, status, and provides sync control buttons including force re-sync functionality

**Details:**

Create `frontend/src/components/SyncStatusPanel.tsx`:

```typescript
import React, { useState } from 'react';
import { Button, Progress, Alert, Tooltip } from '@/components/ui';
import { RefreshCw, AlertCircle, CheckCircle } from 'lucide-react';
import { useSyncStatus } from '@/hooks/useSyncStatus';
import { formatDistanceToNow } from 'date-fns';

interface SyncStatusPanelProps {
  accountId: number;
  folderId?: number;
  onSyncComplete?: () => void;
}

export function SyncStatusPanel({ accountId, folderId, onSyncComplete }: SyncStatusPanelProps) {
  const { isSyncing, emailsSynced, emailsTotal, lastSync, error } = useSyncStatus(accountId, folderId);
  const [isTriggering, setIsTriggering] = useState(false);
  
  const triggerSync = async (force: boolean = false) => {
    if (force && !confirm('This will re-download all emails. Continue?')) return;
    
    setIsTriggering(true);
    try {
      const params = new URLSearchParams({ account_id: accountId.toString() });
      if (folderId) params.append('folder_id', folderId.toString());
      if (force) params.append('force', 'true');
      
      const response = await fetch(`/api/sync/trigger?${params}`, { method: 'POST' });
      if (!response.ok) throw new Error('Failed to trigger sync');
    } catch (err) {
      console.error('Sync trigger failed:', err);
    } finally {
      setIsTriggering(false);
    }
  };
  
  const progressPercent = emailsTotal > 0 ? (emailsSynced / emailsTotal) * 100 : 0;
  
  return (
    <div className="bg-white rounded-lg shadow p-4 space-y-3">
      {/* Sync Status */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {isSyncing ? (
            <>
              <RefreshCw className="h-4 w-4 animate-spin text-blue-500" />
              <span className="text-sm font-medium">Syncing emails...</span>
            </>
          ) : (
            <>
              <CheckCircle className="h-4 w-4 text-green-500" />
              <span className="text-sm text-gray-600">
                {lastSync ? `Last synced ${formatDistanceToNow(lastSync)} ago` : 'Never synced'}
              </span>
            </>
          )}
        </div>
        
        {/* Action Buttons */}
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => triggerSync(false)}
            disabled={isSyncing || isTriggering}
          >
            Sync
          </Button>
          
          <Tooltip content="Re-download all emails">
            <Button
              size="sm"
              variant="outline"
              onClick={() => triggerSync(true)}
              disabled={isSyncing || isTriggering}
              className="text-orange-600 hover:text-orange-700"
            >
              Force Re-sync
            </Button>
          </Tooltip>
        </div>
      </div>
      
      {/* Progress Bar */}
      {isSyncing && emailsTotal > 0 && (
        <div className="space-y-1">
          <div className="flex justify-between text-xs text-gray-600">
            <span>Progress: {emailsSynced.toLocaleString()} / {emailsTotal.toLocaleString()}</span>
            <span>{progressPercent.toFixed(0)}%</span>
          </div>
          <Progress value={progressPercent} className="h-2" />
        </div>
      )}
      
      {/* Error Alert */}
      {error && (
        <Alert variant="destructive" className="text-sm">
          <AlertCircle className="h-4 w-4" />
          <span>Sync error: {error.message}</span>
        </Alert>
      )}
    </div>
  );
}
```

**Test Strategy:**

1. Test component renders correctly with different sync states
2. Verify progress bar shows correct percentage during sync
3. Test sync button triggers API call with correct parameters
4. Test force re-sync shows confirmation dialog and sends force=true
5. Verify buttons are disabled during sync operations
6. Test error display when sync status API fails
7. Test last sync time formatting with various dates
8. Verify component updates in real-time during active sync
