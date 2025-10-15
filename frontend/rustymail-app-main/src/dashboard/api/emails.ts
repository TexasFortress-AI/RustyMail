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

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to send email: ${response.status} ${errorText}`);
    }

    return response.json();
  },
};
