// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Email API client for SMTP operations

import { config } from '../config';

const API_BASE = `${config.api.baseUrl}/dashboard`;
const API_KEY = config.api.apiKey;

export interface SendEmailRequest {
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  body: string;
  body_html?: string;
}

export interface SendEmailResponse {
  success: boolean;
  message_id?: string;
  message: string;
}

export interface DraftReplyRequest {
  email_uid: number;
  folder: string;
  account_id: string;
  instructions?: string;
}

export interface DraftReplyResponse {
  success: boolean;
  data?: {
    draft: string;
    saved_to?: string;
  };
  error?: string;
}

export const emailsApi = {
  // Draft a reply email using AI
  draftReply: async (request: DraftReplyRequest): Promise<DraftReplyResponse> => {
    const url = `${API_BASE}/mcp/execute?variant=high-level`;

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-API-Key': API_KEY,
      },
      body: JSON.stringify({
        tool: 'draft_reply',
        parameters: {
          email_uid: request.email_uid.toString(),
          folder: request.folder,
          account_id: request.account_id,
          instructions: request.instructions,
        },
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to draft reply: ${response.status} ${errorText}`);
    }

    return response.json();
  },

  sendEmail: async (
    request: SendEmailRequest,
    accountEmail?: string
  ): Promise<SendEmailResponse> => {
    const params = new URLSearchParams();
    if (accountEmail) {
      params.append('account_email', accountEmail);
    }

    const url = `${API_BASE}/emails/send${params.toString() ? `?${params}` : ''}`;

    // Add timeout to prevent indefinite hanging (60 seconds for IMAP APPEND + SMTP)
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 60000); // 60 second timeout

    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-API-Key': API_KEY,
        },
        body: JSON.stringify(request),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Failed to send email: ${response.status} ${errorText}`);
      }

      return response.json();
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof Error && error.name === 'AbortError') {
        throw new Error('Email send operation timed out after 60 seconds. The server may be experiencing delays with IMAP operations.');
      }
      throw error;
    }
  },
};
