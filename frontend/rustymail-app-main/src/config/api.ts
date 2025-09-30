// API Configuration
export const API_BASE_URL = 'http://localhost:9437/api';
export const SSE_URL = 'http://localhost:9437/api/dashboard/events';

// API Endpoints
export const API_ENDPOINTS = {
  // Dashboard endpoints
  dashboard: {
    stats: `${API_BASE_URL}/dashboard/stats`,
    clients: `${API_BASE_URL}/dashboard/clients`,
    config: `${API_BASE_URL}/dashboard/config`,
    events: `${API_BASE_URL}/dashboard/events`,
    emails: `${API_BASE_URL}/dashboard/emails`,
    sync: {
      trigger: `${API_BASE_URL}/dashboard/sync/trigger`,
      status: `${API_BASE_URL}/dashboard/sync/status`,
    }
  },

  // Chatbot endpoints
  chatbot: {
    query: `${API_BASE_URL}/dashboard/chatbot/query`,
    stream: `${API_BASE_URL}/dashboard/chatbot/stream`,
  },

  // MCP endpoints
  mcp: {
    tools: `${API_BASE_URL}/dashboard/mcp/tools`,
    execute: `${API_BASE_URL}/dashboard/mcp/execute`,
  },

  // AI endpoints
  ai: {
    providers: `${API_BASE_URL}/dashboard/ai/providers`,
    setProvider: `${API_BASE_URL}/dashboard/ai/providers/set`,
    models: `${API_BASE_URL}/dashboard/ai/models`,
    setModel: `${API_BASE_URL}/dashboard/ai/models/set`,
  },

  // Configuration endpoints
  config: {
    get: `${API_BASE_URL}/dashboard/config`,
    updateImap: `${API_BASE_URL}/dashboard/config/imap`,
    updateRest: `${API_BASE_URL}/dashboard/config/rest`,
    updateDashboard: `${API_BASE_URL}/dashboard/config/dashboard`,
    validate: `${API_BASE_URL}/dashboard/config/validate`,
  }
};

// Request configuration
export const defaultHeaders = {
  'Content-Type': 'application/json',
};

// Helper function for API calls
export async function apiCall<T>(
  url: string,
  options?: RequestInit
): Promise<T> {
  const response = await fetch(url, {
    ...options,
    headers: {
      ...defaultHeaders,
      ...options?.headers,
    },
  });

  if (!response.ok) {
    throw new Error(`API error: ${response.statusText}`);
  }

  return response.json();
}