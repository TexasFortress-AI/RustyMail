
import { 
  DashboardStats, 
  ClientListResponse, 
  ServerConfig,
  ChatbotQuery,
  ChatbotResponse
} from '@/types';
import { 
  generateMockStats, 
  generateMockClients, 
  generateMockConfig,
  handleChatbotQuery
} from './mocks/data';

// Simulate network latency
const simulateNetworkDelay = () => {
  return new Promise(resolve => setTimeout(resolve, Math.random() * 600 + 200));
};

// Simulate random errors (about 5% of requests)
const maybeFailRequest = () => {
  if (Math.random() < 0.05) {
    throw new Error('API request failed. Please try again.');
  }
};

export const apiClient = {
  // Get dashboard statistics
  getStats: async (): Promise<DashboardStats> => {
    await simulateNetworkDelay();
    maybeFailRequest();
    return generateMockStats();
  },

  // Get client list with pagination and optional filtering
  getClients: async (page: number = 1, limit: number = 10, filter?: string): Promise<ClientListResponse> => {
    await simulateNetworkDelay();
    maybeFailRequest();
    return generateMockClients(page, limit, filter);
  },

  // Get server configuration
  getConfig: async (): Promise<ServerConfig> => {
    await simulateNetworkDelay();
    maybeFailRequest();
    return generateMockConfig();
  },

  // Set active IMAP adapter
  setActiveAdapter: async (adapterId: string): Promise<ServerConfig> => {
    await simulateNetworkDelay();
    maybeFailRequest();
    
    const config = generateMockConfig();
    
    // Find the adapter by ID and set it as active
    const newActiveAdapter = config.availableAdapters.find(adapter => adapter.id === adapterId);
    
    if (!newActiveAdapter) {
      throw new Error(`Adapter with ID ${adapterId} not found`);
    }
    
    // Set the new adapter as active
    config.activeAdapter = {
      ...newActiveAdapter,
      isActive: true
    };
    
    // Update the adapter in the available adapters list
    config.availableAdapters = config.availableAdapters.map(adapter => ({
      ...adapter,
      isActive: adapter.id === adapterId
    }));
    
    return config;
  },

  // Send query to chatbot
  queryChatbot: async (query: ChatbotQuery): Promise<ChatbotResponse> => {
    await simulateNetworkDelay();
    maybeFailRequest();
    return handleChatbotQuery(query.query, query.conversation_id);
  }
};

// Initialize EventSource for SSE
export const initEventSource = (
  onStatsUpdated: (data: DashboardStats) => void,
  onClientConnected: (data: any) => void,
  onClientDisconnected: (data: any) => void,
  onSystemAlert: (data: any) => void
) => {
  // In a real app, we'd connect to a real SSE endpoint
  // For demo, we'll simulate events using setInterval
  
  // Simulate stats updates every 10 seconds
  const statsInterval = setInterval(() => {
    onStatsUpdated(generateMockStats());
  }, 10000);
  
  // Simulate client connected events randomly (every 5-15 seconds)
  const clientConnectInterval = setInterval(() => {
    const clientId = `client-${Math.floor(Math.random() * 100) + 1000}`;
    onClientConnected({
      client: {
        id: clientId,
        type: ['SSE', 'API', 'Console'][Math.floor(Math.random() * 3)],
        connectedAt: new Date().toISOString(),
        status: 'Active',
        lastActivity: new Date().toISOString(),
        ipAddress: `192.168.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
      }
    });
  }, Math.random() * 10000 + 5000);
  
  // Simulate client disconnected events randomly (every 7-20 seconds)
  const clientDisconnectInterval = setInterval(() => {
    const clientId = `client-${Math.floor(Math.random() * 100) + 1000}`;
    onClientDisconnected({
      client: {
        id: clientId,
        disconnectedAt: new Date().toISOString(),
        reason: 'Client closed connection'
      }
    });
  }, Math.random() * 13000 + 7000);
  
  // Simulate system alerts rarely (every 30-90 seconds)
  const systemAlertInterval = setInterval(() => {
    const alertTypes = ['info', 'warning', 'error'];
    const alertType = alertTypes[Math.floor(Math.random() * alertTypes.length)];
    const alertMessages = {
      info: [
        'System update completed successfully',
        'New IMAP adapter version available',
        'Cache optimization performed'
      ],
      warning: [
        'High memory usage detected',
        'Connection pool nearing capacity',
        'Slow response time from IMAP server'
      ],
      error: [
        'Failed to connect to IMAP server',
        'Database connection timeout',
        'Authentication service unreachable'
      ]
    };
    
    const message = alertMessages[alertType as keyof typeof alertMessages][
      Math.floor(Math.random() * alertMessages[alertType as keyof typeof alertMessages].length)
    ];
    
    onSystemAlert({
      type: alertType,
      message,
      timestamp: new Date().toISOString()
    });
  }, Math.random() * 60000 + 30000);
  
  // Return a cleanup function
  return () => {
    clearInterval(statsInterval);
    clearInterval(clientConnectInterval);
    clearInterval(clientDisconnectInterval);
    clearInterval(systemAlertInterval);
  };
};
