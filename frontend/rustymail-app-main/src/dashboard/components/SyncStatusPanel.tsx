// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useState } from 'react';
import { Button } from '../../components/ui/button';
import { Progress } from '../../components/ui/progress';
import { API_BASE_URL } from '../../config/api';
import { config } from '../config';
import { useToast } from '../../hooks/use-toast';
import { useSyncStatus } from '../hooks/useSyncStatus';
import { RefreshCw, AlertTriangle } from 'lucide-react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '../../components/ui/alert-dialog';

interface SyncStatusPanelProps {
  accountId: string | undefined;
  folder: string;
  folderLastSync?: string | null;
  onSyncComplete?: () => void;
}

export function SyncStatusPanel({ accountId, folder, folderLastSync, onSyncComplete }: SyncStatusPanelProps) {
  const { toast } = useToast();
  const { isSyncing, emailsSynced, emailsTotal, lastSync, error, startPolling } =
    useSyncStatus(accountId, folder);
  const [triggering, setTriggering] = useState(false);

  const triggerSync = async (force: boolean, allFolders: boolean) => {
    if (!accountId) {
      toast({ title: "No Account", description: "Select an account first", variant: "destructive" });
      return;
    }

    setTriggering(true);
    try {
      const params = new URLSearchParams({ account_id: accountId });
      if (!allFolders) {
        params.set('folder', folder);
      }
      if (force) {
        params.set('force', 'true');
      }

      const response = await fetch(`${API_BASE_URL}/dashboard/sync/trigger?${params}`, {
        method: 'POST',
        headers: { 'X-API-Key': config.api.apiKey },
      });

      if (response.ok) {
        toast({
          title: force ? "Force Re-sync Started" : "Sync Started",
          description: allFolders
            ? "Syncing all folders in background..."
            : `Syncing ${folder} in background...`,
        });
        startPolling();
        // Delay refetch to give sync time to start
        if (onSyncComplete) {
          setTimeout(onSyncComplete, 3000);
        }
      } else {
        const errorText = await response.text();
        toast({ title: "Sync Failed", description: errorText, variant: "destructive" });
      }
    } catch (err) {
      toast({
        title: "Sync Error",
        description: err instanceof Error ? err.message : "Unknown error",
        variant: "destructive",
      });
    } finally {
      setTriggering(false);
    }
  };

  const progressPercent = emailsTotal > 0 ? Math.round((emailsSynced / emailsTotal) * 100) : 0;

  const formatLastSync = (dateStr: string | null) => {
    if (!dateStr) return 'Never';
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      return date.toLocaleDateString();
    } catch {
      return 'Unknown';
    }
  };

  return (
    <div className="flex items-center gap-2">
      {isSyncing ? (
        <>
          <div className="flex items-center gap-2 min-w-[200px]">
            <RefreshCw className="h-3.5 w-3.5 animate-spin text-blue-500" />
            <Progress value={progressPercent} className="h-2 flex-1" />
            <span className="text-xs text-muted-foreground whitespace-nowrap">
              {emailsSynced.toLocaleString()} / {emailsTotal.toLocaleString()}
            </span>
          </div>
        </>
      ) : (
        <>
          <span className="text-xs text-muted-foreground whitespace-nowrap">
            Synced: {formatLastSync(lastSync || folderLastSync || null)}
          </span>

          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => triggerSync(false, false)}
            disabled={triggering}
            title="Fetch only new emails in the current folder since last sync"
          >
            <RefreshCw className={`h-3 w-3 mr-1 ${triggering ? 'animate-spin' : ''}`} />
            Sync
          </Button>

          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => triggerSync(false, true)}
            disabled={triggering}
            title="Fetch new emails across all folders for this account"
          >
            Sync All
          </Button>

          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs text-orange-600 border-orange-300 hover:bg-orange-50"
                disabled={triggering}
                title="Re-download ALL emails in this folder, even ones already cached"
              >
                <AlertTriangle className="h-3 w-3 mr-1" />
                Force Re-sync
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Force Re-sync?</AlertDialogTitle>
                <AlertDialogDescription>
                  This will re-download ALL emails in "{folder}" from the server, even ones already cached.
                  This is useful if emails were missed due to expired tokens or connection errors.
                  It may take a while for large folders.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction onClick={() => triggerSync(true, false)}>
                  Force Re-sync
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </>
      )}

      {error && (
        <span className="text-xs text-destructive" title={error}>
          Error
        </span>
      )}
    </div>
  );
}
