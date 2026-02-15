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
import { RefreshCw, AlertTriangle, ChevronDown, FolderSync } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../../components/ui/dropdown-menu';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
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
  const [forceDialogOpen, setForceDialogOpen] = useState(false);

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
  const effectiveLastSync = lastSync || folderLastSync || null;

  const formatLastSync = (dateStr: string | null) => {
    if (!dateStr) return 'Never';
    try {
      const date = new Date(dateStr);
      const diffMs = Date.now() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      return `${Math.floor(diffHours / 24)}d ago`;
    } catch {
      return 'Unknown';
    }
  };

  // While syncing, show inline progress bar (no dropdown needed)
  if (isSyncing) {
    return (
      <div className="flex items-center gap-2 min-w-[180px]">
        <RefreshCw className="h-3.5 w-3.5 animate-spin text-blue-500 shrink-0" />
        <Progress value={progressPercent} className="h-2 flex-1" />
        <span className="text-xs text-muted-foreground whitespace-nowrap">
          {emailsSynced.toLocaleString()} / {emailsTotal.toLocaleString()}
        </span>
      </div>
    );
  }

  // When idle, show compact dropdown trigger with sync actions inside
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs gap-1"
            disabled={triggering}
          >
            <RefreshCw className={`h-3 w-3 ${triggering ? 'animate-spin' : ''}`} />
            {formatLastSync(effectiveLastSync)}
            <ChevronDown className="h-3 w-3 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-64">
          <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
            Last synced: {formatLastSync(effectiveLastSync)}
            {error && <span className="text-destructive ml-1">(error)</span>}
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onClick={() => triggerSync(false, false)}
            disabled={triggering}
          >
            <RefreshCw className="h-4 w-4 mr-2" />
            <div>
              <div className="font-medium">Sync folder</div>
              <div className="text-xs text-muted-foreground">Fetch new emails in {folder}</div>
            </div>
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() => triggerSync(false, true)}
            disabled={triggering}
          >
            <FolderSync className="h-4 w-4 mr-2" />
            <div>
              <div className="font-medium">Sync all folders</div>
              <div className="text-xs text-muted-foreground">Fetch new emails across all folders</div>
            </div>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onClick={() => setForceDialogOpen(true)}
            disabled={triggering}
            className="text-orange-600 focus:text-orange-600"
          >
            <AlertTriangle className="h-4 w-4 mr-2" />
            <div>
              <div className="font-medium">Force re-sync</div>
              <div className="text-xs text-muted-foreground">Re-download all emails in {folder}</div>
            </div>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <AlertDialog open={forceDialogOpen} onOpenChange={setForceDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Force Re-sync?</AlertDialogTitle>
            <AlertDialogDescription>
              This will re-download ALL emails in "{folder}" from the server,
              even ones already cached. This is useful if emails were missed
              due to expired tokens or connection errors. It may take a while
              for large folders.
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
  );
}
