# Task ID: 75

**Title:** Create useSyncStatus hook for frontend

**Status:** done

**Dependencies:** 72 ✓

**Priority:** medium

**Description:** Implement a React hook that polls the sync status API and provides real-time sync progress updates to UI components

**Details:**

Create `frontend/src/hooks/useSyncStatus.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';

interface SyncStatus {
  status: string;
  last_sync: string | null;
  emails_synced: number;
  emails_total: number;
  is_syncing: boolean;
}

interface UseSyncStatusReturn {
  isSyncing: boolean;
  emailsSynced: number;
  emailsTotal: number;
  lastSync: Date | null;
  error: Error | null;
  refetch: () => void;
}

export function useSyncStatus(accountId: number, folderId?: number): UseSyncStatusReturn {
  const [pollInterval, setPollInterval] = useState<number | false>(false);
  
  const { data, error, refetch } = useQuery<SyncStatus>({
    queryKey: ['syncStatus', accountId, folderId],
    queryFn: async () => {
      const params = new URLSearchParams({ account_id: accountId.toString() });
      if (folderId) params.append('folder_id', folderId.toString());
      
      const response = await fetch(`/api/sync/status?${params}`);
      if (!response.ok) throw new Error('Failed to fetch sync status');
      return response.json();
    },
    refetchInterval: pollInterval,
    refetchIntervalInBackground: true,
  });
  
  // Enable polling when syncing, disable when idle
  useEffect(() => {
    if (data?.is_syncing) {
      setPollInterval(2000); // Poll every 2 seconds during sync
    } else {
      setPollInterval(false); // Stop polling when idle
    }
  }, [data?.is_syncing]);
  
  return {
    isSyncing: data?.is_syncing ?? false,
    emailsSynced: data?.emails_synced ?? 0,
    emailsTotal: data?.emails_total ?? 0,
    lastSync: data?.last_sync ? new Date(data.last_sync) : null,
    error: error as Error | null,
    refetch,
  };
}
```

Export from hooks index file for easy importing.

**Test Strategy:**

1. Create test component that uses the hook and verify it renders sync status
2. Mock API responses to test different sync states (idle, syncing, error)
3. Verify polling starts when is_syncing=true and stops when is_syncing=false
4. Test error handling when API calls fail
5. Verify memory leaks don't occur when component unmounts during active polling
6. Test with different account/folder ID combinations
