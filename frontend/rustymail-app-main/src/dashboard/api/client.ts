// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.


import {
  DashboardStats,
  ClientListResponse,
  ServerConfig,
  ChatbotQuery,
  ChatbotResponse
} from '@/types';

// Base API URL - in production this would come from environment variables
const API_BASE = '/api/dashboard';

// Utility function to handle API requests
const apiRequest = async <T>(url: string, options?: RequestInit): Promise<T> => {
  const response = await fetch(url, {
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
    ...options,
  });

  if (!response.ok) {
    throw new Error(`API request failed: ${response.status} ${response.statusText}`);
  }

  return response.json();
};

export const apiClient = {
  // Get dashboard statistics
  getStats: async (): Promise<DashboardStats> => {
    const data = await apiRequest<any>(`${API_BASE}/stats`);

    // Transform backend data to frontend format
    return {
      activeConnections: data.active_dashboard_sse_clients || 0,
      requestsPerMinute: data.requests_per_minute || 0,
      averageResponseTime: data.average_response_time_ms || 0,
      systemHealth: {
        status: data.system_health?.status || 'unknown',
        cpuUsage: Math.round(data.system_health?.cpu_usage || 0),
        memoryUsage: Math.round(data.system_health?.memory_usage || 0),
      },
      requestRate: [
        // Generate mock chart data for now since backend doesn't provide historical data yet
        { timestamp: new Date(Date.now() - 120*60*1000), value: Math.max(0, data.requests_per_minute - 5) },
        { timestamp: new Date(Date.now() - 60*60*1000), value: Math.max(0, data.requests_per_minute - 2) },
        { timestamp: new Date(), value: data.requests_per_minute || 0 },
      ],
      lastUpdated: data.last_updated || new Date().toISOString(),
    };
  },

  // Get client list with pagination and optional filtering
  getClients: async (page: number = 1, limit: number = 10, filter?: string): Promise<ClientListResponse> => {
    const params = new URLSearchParams({
      page: page.toString(),
      limit: limit.toString(),
    });

    if (filter) {
      params.append('filter', filter);
    }

    const data = await apiRequest<any>(`${API_BASE}/clients?${params}`);

    // Transform backend data to frontend format
    return {
      clients: data.clients.map((client: any) => ({
        id: client.id,
        type: client.type,
        ipAddress: client.ip_address,
        userAgent: client.user_agent || 'Unknown',
        connectedAt: client.connected_at,
        lastActivity: client.last_activity,
        status: client.status,
      })),
      pagination: {
        total: data.pagination.total,
        page: data.pagination.page,
        limit: data.pagination.limit,
        totalPages: data.pagination.total_pages,
      },
    };
  },

  // Get server configuration
  getConfig: async (): Promise<ServerConfig> => {
    const data = await apiRequest<any>(`${API_BASE}/config`);

    // Transform backend data to frontend format
    return {
      activeAdapter: {
        id: 'current',
        name: `IMAP (${data.imap.host}:${data.imap.port})`,
        type: 'imap',
        host: data.imap.host,
        port: data.imap.port,
        username: data.imap.user,
        isActive: true,
      },
      availableAdapters: [
        {
          id: 'current',
          name: `IMAP (${data.imap.host}:${data.imap.port})`,
          type: 'imap',
          host: data.imap.host,
          port: data.imap.port,
          username: data.imap.user,
          isActive: true,
        }
      ],
      settings: {
        dashboard: {
          enabled: data.dashboard?.enabled || false,
          port: data.dashboard?.port || parseInt(import.meta.env.VITE_DASHBOARD_PORT || '0'),
        },
        rest: {
          enabled: data.rest?.enabled || false,
          host: data.rest?.host || import.meta.env.VITE_REST_HOST || '',
          port: data.rest?.port || parseInt(import.meta.env.VITE_REST_PORT || '0'),
        }
      }
    };
  },

  // Set active IMAP adapter - for now just return current config since backend doesn't support multiple adapters yet
  setActiveAdapter: async (adapterId: string): Promise<ServerConfig> => {
    // For now, just return the current config since backend doesn't support switching adapters yet
    return apiClient.getConfig();
  },

  // Send query to chatbot
  queryChatbot: async (query: ChatbotQuery): Promise<ChatbotResponse> => {
    const response = await apiRequest<any>(`${API_BASE}/chatbot/query`, {
      method: 'POST',
      body: JSON.stringify({
        query: query.query,
        conversation_id: query.conversation_id,
        provider_override: query.provider_override,
        model_override: query.model_override,
        current_folder: query.current_folder,
        account_id: query.account_id,
        enabled_tools: query.enabled_tools,
      }),
    });

    return {
      text: response.text,
      conversationId: response.conversation_id,
      emailData: response.email_data,
      followupSuggestions: response.followup_suggestions || [],
    };
  },

  // AI Provider Management
  getAiProviders: async (): Promise<{
    currentProvider: string | null;
    availableProviders: Array<{
      name: string;
      provider_type: string;
      model: string;
      priority: number;
      enabled: boolean;
    }>;
  }> => {
    const response = await apiRequest<any>(`${API_BASE}/ai/providers`);
    return {
      currentProvider: response.current_provider,
      availableProviders: response.available_providers,
    };
  },

  setAiProvider: async (providerName: string): Promise<{ message: string; current_provider: string }> => {
    return await apiRequest<any>(`${API_BASE}/ai/providers/set`, {
      method: 'POST',
      body: JSON.stringify({
        provider_name: providerName,
      }),
    });
  },

  // AI Model Management
  getAiModels: async (): Promise<{
    currentModel: string | null;
    availableModels: string[];
    provider: string | null;
  }> => {
    const response = await apiRequest<any>(`${API_BASE}/ai/models`);
    // Transform snake_case from backend to camelCase for frontend
    return {
      currentModel: response.current_model,
      availableModels: response.available_models,
      provider: response.provider,
    };
  },

  setAiModel: async (modelName: string): Promise<{ message: string }> => {
    return await apiRequest<any>(`${API_BASE}/ai/models/set`, {
      method: 'POST',
      body: JSON.stringify({
        model_name: modelName,
      }),
    });
  },

  // MCP Tools
  getMcpTools: async (variant: 'low-level' | 'high-level'): Promise<{ tools: McpTool[] }> => {
    return await apiRequest<{ tools: McpTool[] }>(`${API_BASE}/mcp/tools?variant=${variant}`);
  },
};

// Initialize EventSource for SSE
export const initEventSource = (
  onStatsUpdated: (data: DashboardStats) => void,
  onClientConnected: (data: any) => void,
  onClientDisconnected: (data: any) => void,
  onSystemAlert: (data: any) => void,
  onReconnected?: () => void
) => {
  // Connect to the real SSE endpoint
  // EventSource will automatically reconnect and send Last-Event-ID header
  const eventSource = new EventSource(`${API_BASE}/events`);

  let isReconnection = false;
  let connectionAttempts = 0;

  // Handle different event types
  eventSource.addEventListener('welcome', (event) => {
    try {
      const welcomeData = JSON.parse(event.data);
      if (welcomeData.reconnect) {
        console.log('SSE reconnected with client ID:', welcomeData.clientId);
        isReconnection = true;
        if (onReconnected) {
          onReconnected();
        }
      } else {
        console.log('SSE connected with client ID:', welcomeData.clientId);
      }
      connectionAttempts = 0; // Reset on successful connection
    } catch (e) {
      console.log('SSE connected:', event.data);
    }
  });

  eventSource.addEventListener('stats_update', (event) => {
    try {
      const data = JSON.parse(event.data);
      // Transform backend data to frontend format (same as getStats)
      const transformedData = {
        activeConnections: data.active_dashboard_sse_clients || 0,
        requestsPerMinute: data.requests_per_minute || 0,
        averageResponseTime: data.average_response_time_ms || 0,
        systemHealth: {
          status: data.system_health?.status || 'unknown',
          cpuUsage: Math.round(data.system_health?.cpu_usage || 0),
          memoryUsage: Math.round(data.system_health?.memory_usage || 0),
        },
        requestRate: [
          { timestamp: new Date(Date.now() - 120*60*1000), value: Math.max(0, data.requests_per_minute - 5) },
          { timestamp: new Date(Date.now() - 60*60*1000), value: Math.max(0, data.requests_per_minute - 2) },
          { timestamp: new Date(), value: data.requests_per_minute || 0 },
        ],
        lastUpdated: data.last_updated || new Date().toISOString(),
      };
      onStatsUpdated(transformedData);
    } catch (e) {
      console.error('Error parsing stats update:', e);
    }
  });

  eventSource.addEventListener('client_connected', (event) => {
    try {
      const data = JSON.parse(event.data);
      onClientConnected(data);
    } catch (e) {
      console.error('Error parsing client connected event:', e);
    }
  });

  eventSource.addEventListener('client_disconnected', (event) => {
    try {
      const data = JSON.parse(event.data);
      onClientDisconnected(data);
    } catch (e) {
      console.error('Error parsing client disconnected event:', e);
    }
  });

  eventSource.addEventListener('system_alert', (event) => {
    try {
      const data = JSON.parse(event.data);
      onSystemAlert(data);
    } catch (e) {
      console.error('Error parsing system alert event:', e);
    }
  });

  eventSource.addEventListener('error', (event) => {
    connectionAttempts++;
    if (eventSource.readyState === EventSource.CONNECTING) {
      console.log(`SSE reconnecting... (attempt ${connectionAttempts})`);
    } else if (eventSource.readyState === EventSource.CLOSED) {
      console.error('SSE connection closed permanently');
    } else {
      console.error('SSE error:', event);
    }
  });

  // EventSource connection states for reference:
  // - CONNECTING (0): Connecting or reconnecting
  // - OPEN (1): Connected
  // - CLOSED (2): Connection closed and won't retry

  // Fallback: Fetch stats every 30 seconds as backup
  const fallbackStatsInterval = setInterval(async () => {
    try {
      const stats = await apiClient.getStats();
      onStatsUpdated(stats);
    } catch (error) {
      console.error('Error fetching stats fallback:', error);
    }
  }, 30000);

  // Return a cleanup function
  return () => {
    eventSource.close();
    clearInterval(fallbackStatsInterval);
  };
};
