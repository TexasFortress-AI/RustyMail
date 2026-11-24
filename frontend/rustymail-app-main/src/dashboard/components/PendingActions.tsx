// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Actions the assistant asked for that only the owner can run (send, delete,
// move...). The server did not run them; it recorded exactly what would run
// and looked up the messages involved. Confirm runs exactly that; nothing the
// assistant writes can confirm it.

import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { ShieldAlert } from 'lucide-react';
import { API_BASE_URL } from '@/config/api';
import { authFetch } from '@/dashboard/api/authFetch';
import type { PendingAction } from '@/types';

type Status = 'pending' | 'running' | 'done' | 'failed' | 'dismissed';

function PendingActionCard({ action }: { action: PendingAction }) {
  const [status, setStatus] = useState<Status>('pending');
  const [detail, setDetail] = useState('');

  const confirm = async () => {
    setStatus('running');
    try {
      const response = await authFetch(
        `${API_BASE_URL}/dashboard/mcp/execute?variant=${encodeURIComponent(action.variant)}`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ tool: action.tool, parameters: action.parameters }),
        },
      );
      const result = await response.json();
      setStatus(result.success ? 'done' : 'failed');
      setDetail(result.success ? 'Done.' : String(result.error ?? result.message ?? 'Failed'));
    } catch (error) {
      setStatus('failed');
      setDetail(String(error));
    }
  };

  return (
    <div className="mt-2 rounded-md border border-amber-500/50 bg-amber-500/10 p-2 text-xs" data-testid="pending-action">
      <div className="flex items-center gap-1 font-medium">
        <ShieldAlert className="h-3.5 w-3.5" />
        Needs your confirmation: <code>{action.tool}</code>
      </div>
      {action.affected.length > 0 && (
        <ul className="mt-1 list-disc pl-4">
          {action.affected.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
      <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded bg-background/60 p-1">
        {JSON.stringify(action.parameters, null, 2)}
      </pre>
      {status === 'pending' && (
        <div className="mt-2 flex gap-2">
          <Button size="sm" variant="destructive" onClick={confirm} data-testid="pending-action-confirm">
            Confirm
          </Button>
          <Button size="sm" variant="outline" onClick={() => setStatus('dismissed')}>
            Dismiss
          </Button>
        </div>
      )}
      {status === 'running' && <div className="mt-2">Running…</div>}
      {status === 'dismissed' && <div className="mt-2">Dismissed; nothing was run.</div>}
      {(status === 'done' || status === 'failed') && <div className="mt-2">{detail}</div>}
    </div>
  );
}

export function PendingActions({ actions }: { actions: PendingAction[] }) {
  return (
    <div>
      {actions.map((action) => (
        <PendingActionCard key={action.id} action={action} />
      ))}
    </div>
  );
}
