// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Every /api/dashboard request must carry the API key. These helpers add it,
// including for downloads and images, which the browser would otherwise
// request without custom headers.

import { config } from '../config';

export const API_KEY_HEADER = 'X-API-Key';
// How long a download's object URL stays alive after the click starts it.
const DOWNLOAD_URL_LIFETIME_MS = 60_000;

/** The given headers plus the API key. */
export function authHeaders(headers?: HeadersInit): Headers {
  const merged = new Headers(headers);
  merged.set(API_KEY_HEADER, config.api.apiKey);
  return merged;
}

/** `fetch` with the API key added to the request headers. */
export function authFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  return fetch(input, { ...init, headers: authHeaders(init.headers) });
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
