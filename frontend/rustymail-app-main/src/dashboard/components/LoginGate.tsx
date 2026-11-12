// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Shows the dashboard only with a live session; otherwise asks for the API
// key once. After a successful login the page reloads, so every component
// (and the live-update stream) starts fresh with the new session.

import React, { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { UNAUTHORIZED_EVENT } from '../api/authFetch';
import { hasSession, logIn } from '../api/session';

type GateState = 'checking' | 'signed-in' | 'signed-out';

const LoginGate: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, setState] = useState<GateState>('checking');
  const [apiKey, setApiKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    hasSession()
      .then((live) => setState(live ? 'signed-in' : 'signed-out'))
      .catch(() => setState('signed-out'));
    const onUnauthorized = () => setState('signed-out');
    window.addEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
    return () => window.removeEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
  }, []);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      if (await logIn(apiKey)) {
        window.location.reload();
        return;
      }
      setError('That key was not accepted.');
    } catch {
      setError('Could not reach the RustyMail server.');
    }
    setApiKey('');
    setSubmitting(false);
  };

  if (state === 'signed-in') return <>{children}</>;
  if (state === 'checking') return null;
  return (
    <div className="min-h-screen flex items-center justify-center bg-background p-4">
      <Card className="w-full max-w-sm" data-testid="login-gate">
        <CardHeader>
          <CardTitle>Sign in to RustyMail</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="rustymail-api-key">API key</Label>
              <Input
                id="rustymail-api-key"
                type="password"
                autoComplete="current-password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                autoFocus
              />
              <p className="text-xs text-muted-foreground">
                The value of RUSTYMAIL_API_KEY. It is sent once and never stored in the browser.
              </p>
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" className="w-full" disabled={submitting || !apiKey}>
              Sign in
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
};

export default LoginGate;
