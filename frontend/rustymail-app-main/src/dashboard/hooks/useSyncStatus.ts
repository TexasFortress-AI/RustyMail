// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { useState, useEffect, useRef, useCallback } from 'react';
import { API_BASE_URL } from '../../config/api';
import { config } from '../config';

const POLL_INTERVAL_MS = 2000;

interface SyncStatusResponse {
  folder: string;
  status: string;
  last_uid_synced: number | null;
  last_full_sync: string | null;
  last_incremental_sync: string | null;
  error_message: string | null;
  emails_synced: number;
  emails_total: number;
}

interface SyncStatusState {
  isSyncing: boolean;
  emailsSynced: number;
  emailsTotal: number;
  lastSync: string | null;
  error: string | null;
}

export function useSyncStatus(accountId: string | undefined, folder: string) {
  const [status, setStatus] = useState<SyncStatusState>({
    isSyncing: false,
    emailsSynced: 0,
    emailsTotal: 0,
    lastSync: null,
    error: null,
  });

  const pollingRef = useRef(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchStatus = useCallback(async () => {
    if (!accountId) return;

    try {
      const response = await fetch(
        `${API_BASE_URL}/dashboard/sync/status?account_id=${accountId}&folder=${encodeURIComponent(folder)}`,
        { headers: { 'X-API-Key': config.api.apiKey } }
      );

      if (!response.ok) return;

      const data: SyncStatusResponse = await response.json();
      const isSyncing = data.status === 'Syncing';

      setStatus({
        isSyncing,
        emailsSynced: data.emails_synced,
        emailsTotal: data.emails_total,
        lastSync: data.last_incremental_sync,
        error: data.error_message,
      });

      // Start polling when syncing, stop when idle
      if (isSyncing && !pollingRef.current) {
        pollingRef.current = true;
        intervalRef.current = setInterval(fetchStatus, POLL_INTERVAL_MS);
      } else if (!isSyncing && pollingRef.current) {
        pollingRef.current = false;
        if (intervalRef.current) {
          clearInterval(intervalRef.current);
          intervalRef.current = null;
        }
      }
    } catch (err) {
      setStatus(prev => ({
        ...prev,
        error: err instanceof Error ? err.message : 'Failed to fetch sync status',
      }));
    }
  }, [accountId, folder]);

  // Fetch on mount and when dependencies change
  useEffect(() => {
    fetchStatus();
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      pollingRef.current = false;
    };
  }, [fetchStatus]);

  // Allow external trigger to start polling (e.g., after triggering sync)
  const startPolling = useCallback(() => {
    if (!pollingRef.current) {
      pollingRef.current = true;
      fetchStatus();
      intervalRef.current = setInterval(fetchStatus, POLL_INTERVAL_MS);
    }
  }, [fetchStatus]);

  return { ...status, startPolling, refetchStatus: fetchStatus };
}
