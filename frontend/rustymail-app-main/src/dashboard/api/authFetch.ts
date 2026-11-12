// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Every /api/dashboard request is authenticated by the session cookie set at
// login (HttpOnly, so page scripts never see a credential). These helpers
// make sure the cookie is sent, and announce a 401 so the login screen can
// appear. Downloads and inline images are fetched here too, so they go
// through the same path.

/** Fired on window when the dashboard API answers 401 (no or expired session). */
export const UNAUTHORIZED_EVENT = 'rustymail:unauthorized';
// How long a download's object URL stays alive after the click starts it.
const DOWNLOAD_URL_LIFETIME_MS = 60_000;
const HTTP_UNAUTHORIZED = 401;

/** Tells the login screen that the session is missing or has expired. */
export function announceUnauthorized(): void {
  window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
}

/** `fetch` for the dashboard API: sends the session cookie, announces 401s. */
export async function authFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  const response = await fetch(input, { ...init, credentials: 'same-origin' });
  if (response.status === HTTP_UNAUTHORIZED) announceUnauthorized();
  return response;
}

/** Fetches a protected file and returns an object URL for it (caller revokes it). */
export async function fetchObjectUrl(url: string): Promise<string> {
  const response = await authFetch(url);
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status} ${response.statusText}`);
  }
  return URL.createObjectURL(await response.blob());
}

/** The filename from a Content-Disposition header, if it names one. */
function dispositionFilename(response: Response): string | null {
  const disposition = response.headers.get('Content-Disposition') || '';
  const encoded = /filename\*=UTF-8''([^;]+)/i.exec(disposition);
  if (encoded) return decodeURIComponent(encoded[1]);
  const plain = /filename="?([^";]+)"?/i.exec(disposition);
  return plain ? plain[1] : null;
}

/** Downloads a protected file, saving it under the server's filename if it gives one. */
export async function downloadWithAuth(url: string, fallbackFilename: string): Promise<void> {
  const response = await authFetch(url);
  if (!response.ok) {
    throw new Error(`Download failed: ${response.status} ${response.statusText}`);
  }
  const objectUrl = URL.createObjectURL(await response.blob());
  const link = document.createElement('a');
  link.href = objectUrl;
  link.download = dispositionFilename(response) || fallbackFilename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(objectUrl), DOWNLOAD_URL_LIFETIME_MS);
}
