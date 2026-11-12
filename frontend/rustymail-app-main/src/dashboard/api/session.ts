// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Dashboard login. The API key is sent once, in a POST body; the server
// answers with an HttpOnly session cookie, so the key is never kept in the
// page, the bundle or browser storage.

import config from '../config';

const SESSION_URL = `${config.api.baseUrl}/dashboard/session`;

/** True when the browser already has a live session. */
export async function hasSession(): Promise<boolean> {
  const response = await fetch(SESSION_URL, { credentials: 'same-origin' });
  return response.ok;
}

/** Exchanges the API key for a session cookie. False when the key is wrong. */
export async function logIn(apiKey: string): Promise<boolean> {
  const response = await fetch(SESSION_URL, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ api_key: apiKey }),
  });
  return response.ok;
}

/** Ends the session on the server and clears the cookie. */
export async function logOut(): Promise<void> {
  await fetch(SESSION_URL, { method: 'DELETE', credentials: 'same-origin' });
}
