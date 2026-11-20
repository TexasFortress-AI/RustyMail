// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

// Providers whose server owns every inference setting: RustyMail sends none,
// so the dashboard offers nothing to configure for them.
const SERVER_MANAGED_PROVIDERS = ['model_serve'];

export function isServerManaged(provider: string): boolean {
  return SERVER_MANAGED_PROVIDERS.includes(provider);
}

export function ServerManagedSettings({ provider }: { provider: string }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Settings managed by {provider}</CardTitle>
        <CardDescription>
          {provider} sets the context size, keep-alive, thinking and sampling for each of its models, so one
          loaded copy of a model serves every caller. RustyMail sends only the model and the messages. Change
          these settings in {provider}'s own configuration.
        </CardDescription>
      </CardHeader>
    </Card>
  );
}
