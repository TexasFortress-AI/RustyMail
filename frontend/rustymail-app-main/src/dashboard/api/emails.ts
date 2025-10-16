// Email API client for SMTP operations

const API_BASE = '/api/dashboard';

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

export const emailsApi = {
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
