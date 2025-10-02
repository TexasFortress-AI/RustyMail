import { Account, AutoConfigResult, AccountFormData } from '../../types';
import config from '../config';

const API_BASE = `${config.api.baseUrl}/dashboard`;
const API_KEY = config.api.apiKey;

const headers = {
  'Content-Type': 'application/json',
  'X-API-Key': API_KEY,
};

export const accountsApi = {
  // Auto-configure account settings from email address
  async autoConfig(emailAddress: string): Promise<AutoConfigResult> {
    const response = await fetch(`${API_BASE}/accounts/auto-config`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ email_address: emailAddress }),
    });

    if (!response.ok) {
      throw new Error(`Auto-config failed: ${response.statusText}`);
    }

    return response.json();
  },

  // List all accounts
  async listAccounts(): Promise<Account[]> {
    const response = await fetch(`${API_BASE}/accounts`, {
      method: 'GET',
      headers,
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch accounts: ${response.statusText}`);
    }

    const data = await response.json();
    return data.accounts || data;
  },

  // Get default account
  async getDefaultAccount(): Promise<Account | null> {
    const response = await fetch(`${API_BASE}/accounts/default`, {
      method: 'GET',
      headers,
    });

    if (!response.ok) {
      if (response.status === 404) {
        return null;
      }
      throw new Error(`Failed to fetch default account: ${response.statusText}`);
    }

    return response.json();
  },

  // Get account by ID
  async getAccount(id: string): Promise<Account> {
    const response = await fetch(`${API_BASE}/accounts/${id}`, {
      method: 'GET',
      headers,
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch account: ${response.statusText}`);
    }

    return response.json();
  },

  // Create new account
  async createAccount(data: AccountFormData): Promise<{ id: string }> {
    const response = await fetch(`${API_BASE}/accounts`, {
      method: 'POST',
      headers,
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: response.statusText }));
      throw new Error(error.message || 'Failed to create account');
    }

    return response.json();
  },

  // Update account
  async updateAccount(id: string, data: Partial<AccountFormData>): Promise<void> {
    const response = await fetch(`${API_BASE}/accounts/${id}`, {
      method: 'PUT',
      headers,
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: response.statusText }));
      throw new Error(error.message || 'Failed to update account');
    }
  },

  // Delete account
  async deleteAccount(id: string): Promise<void> {
    const response = await fetch(`${API_BASE}/accounts/${id}`, {
      method: 'DELETE',
      headers,
    });

    if (!response.ok) {
      throw new Error(`Failed to delete account: ${response.statusText}`);
    }
  },

  // Set as default account
  async setDefaultAccount(id: string): Promise<void> {
    const response = await fetch(`${API_BASE}/accounts/${id}/default`, {
      method: 'POST',
      headers,
    });

    if (!response.ok) {
      throw new Error(`Failed to set default account: ${response.statusText}`);
    }
  },

  // Validate connection
  async validateConnection(id: string): Promise<{ success: boolean; message?: string }> {
    const response = await fetch(`${API_BASE}/accounts/${id}/validate`, {
      method: 'POST',
      headers,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: response.statusText }));
      return {
        success: false,
        message: error.message || 'Connection validation failed',
      };
    }

    return { success: true };
  },
};
