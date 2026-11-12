// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// A Server-Sent Events client for the dashboard's live updates.
//
// It keeps EventSource's interface (addEventListener, close, readyState) but
// reads the stream with fetch, so a lost session (401) is announced to the
// login screen instead of failing silently. Like EventSource it sends
// Last-Event-ID when reconnecting and honours the server's `retry:` delay.
// It gives up only when the session is refused (401/403); dropped
// connections and other errors, such as the dev proxy's 5xx while the
// backend restarts, are retried.

import { announceUnauthorized } from './authFetch';

// EventSource's reconnection delay when the server sends no `retry:` field.
const DEFAULT_RETRY_MS = 3000;
// Responses that retrying cannot fix: the session is missing or expired.
const AUTH_FAILURE_STATUSES = [401, 403];

export interface StreamEvent {
  type: string;
  data: string;
  lastEventId: string;
}

type StreamListener = (event: StreamEvent) => void;

export class AuthenticatedEventStream {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;

  readyState = AuthenticatedEventStream.CONNECTING;
  private listeners = new Map<string, StreamListener[]>();
  private controller = new AbortController();
  private lastEventId = '';
  private retryMs = DEFAULT_RETRY_MS;
  private retryTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly url: string) {
    void this.connect();
  }

  addEventListener(type: string, listener: StreamListener): void {
    this.listeners.set(type, [...(this.listeners.get(type) || []), listener]);
  }

  close(): void {
    this.readyState = AuthenticatedEventStream.CLOSED;
    clearTimeout(this.retryTimer);
    this.controller.abort();
  }

  private emit(type: string, data = ''): void {
    const event = { type, data, lastEventId: this.lastEventId };
    (this.listeners.get(type) || []).forEach((listener) => listener(event));
  }

  private async connect(): Promise<void> {
    const headers = new Headers({ Accept: 'text/event-stream' });
    if (this.lastEventId) headers.set('Last-Event-ID', this.lastEventId);
    let response: Response;
    try {
      response = await fetch(this.url, {
        headers,
        cache: 'no-store',
        credentials: 'same-origin',
        signal: this.controller.signal,
      });
    } catch {
      return this.scheduleReconnect();
    }
    if (AUTH_FAILURE_STATUSES.includes(response.status)) {
      this.close();
      announceUnauthorized();
      return this.emit('error');
    }
    if (!response.ok || !response.body) {
      return this.scheduleReconnect();
    }
    this.readyState = AuthenticatedEventStream.OPEN;
    this.emit('open');
    try {
      await this.readEvents(response.body);
    } catch {
      // The connection dropped; fall through to reconnect.
    }
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.readyState === AuthenticatedEventStream.CLOSED) return;
    this.readyState = AuthenticatedEventStream.CONNECTING;
    this.emit('error');
    this.retryTimer = setTimeout(() => void this.connect(), this.retryMs);
  }

  private async readEvents(body: ReadableStream<Uint8Array>): Promise<void> {
    const reader = body.pipeThrough(new TextDecoderStream()).getReader();
    const pending = { type: '', data: [] as string[] };
    let buffer = '';
    for (;;) {
      const { value, done } = await reader.read();
      if (done) return;
      buffer += value;
      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';
      lines.forEach((line) => this.handleLine(line.replace(/\r$/, ''), pending));
    }
  }

  // One line of the SSE wire format (https://html.spec.whatwg.org/#event-stream-interpretation).
  private handleLine(line: string, pending: { type: string; data: string[] }): void {
    if (line === '') {
      if (pending.data.length > 0) this.emit(pending.type || 'message', pending.data.join('\n'));
      pending.type = '';
      pending.data = [];
      return;
    }
    if (line.startsWith(':')) return; // comment / keep-alive
    const colon = line.indexOf(':');
    const field = colon < 0 ? line : line.slice(0, colon);
    const value = colon < 0 ? '' : line.slice(colon + 1).replace(/^ /, '');
    if (field === 'event') pending.type = value;
    else if (field === 'data') pending.data.push(value);
    else if (field === 'id' && !value.includes('\0')) this.lastEventId = value;
    else if (field === 'retry' && /^\d+$/.test(value)) this.retryMs = Number(value);
  }
}
