
// Stats Types
export interface DashboardStats {
  activeConnections: number;
  requestRate: RequestRateData[];
  systemHealth: SystemHealth;
  lastUpdated: string; // ISO timestamp
}

export interface RequestRateData {
  timestamp: string;
  value: number;
}

export interface SystemHealth {
  status: 'healthy' | 'degraded' | 'critical';
  memoryUsage: number; // percentage
  cpuUsage: number; // percentage
}

// Client Types
export interface ClientInfo {
  id: string;
  type: 'SSE' | 'API' | 'Console';
  connectedAt: string; // ISO timestamp
  status: 'Active' | 'Idle' | 'Disconnecting';
  lastActivity: string; // ISO timestamp
  ipAddress?: string;
  userAgent?: string;
}

export interface ClientListResponse {
  clients: ClientInfo[];
  pagination: {
    total: number;
    page: number;
    limit: number;
    totalPages: number;
  }
}

// Config Types
export interface ServerConfig {
  activeAdapter: ImapAdapter;
  availableAdapters: ImapAdapter[];
  version: string;
  uptime: number; // seconds
}

export interface ImapAdapter {
  id: string;
  name: string;
  description: string;
  isActive: boolean;
}

// Chatbot Types
export interface ChatbotQuery {
  query: string;
  conversation_id?: string;
  current_folder?: string;
  account_id?: string;
}

export interface ChatbotResponse {
  text: string;
  conversation_id: string;
  emailData?: EmailData;
  followupSuggestions?: string[];
}

export interface ChatMessage {
  id: string;
  type: 'user' | 'ai';
  text: string;
  timestamp: string;
  emailData?: EmailData;
  followupSuggestions?: string[];
}

export interface EmailData {
  // Various email-related data returned by chatbot
  messages?: EmailMessage[];
  count?: number;
  folders?: EmailFolder[];
}

export interface EmailMessage {
  id: string;
  subject: string;
  from: string;
  date: string;
  snippet: string;
  isRead: boolean;
}

export interface EmailFolder {
  name: string;
  count: number;
  unreadCount: number;
}

// SSE Event Types
export interface SSEEvent {
  type: 'stats_updated' | 'client_connected' | 'client_disconnected' | 'system_alert';
  timestamp: Date;
  data: any;
}

// Account Types
export interface Account {
  id: string;
  account_name: string;
  email_address: string;
  provider_type?: string;
  imap_host: string;
  imap_port: number;
  imap_user: string;
  imap_use_tls: boolean;
  smtp_host?: string;
  smtp_port?: number;
  smtp_user?: string;
  smtp_use_tls?: boolean;
  smtp_use_starttls?: boolean;
  is_active: boolean;
  is_default: boolean;
}

export interface AutoConfigResult {
  provider_found: boolean;
  provider_type?: string;
  display_name?: string;
  imap_host?: string;
  imap_port?: number;
  imap_use_tls?: boolean;
  smtp_host?: string;
  smtp_port?: number;
  smtp_use_tls?: boolean;
  smtp_use_starttls?: boolean;
  supports_oauth: boolean;
  oauth_provider?: string;
}

export interface AccountFormData {
  account_name: string;
  email_address: string;
  provider_type?: string;
  imap_host: string;
  imap_port: number;
  imap_user: string;
  imap_pass: string;
  imap_use_tls: boolean;
  smtp_host?: string;
  smtp_port?: number;
  smtp_user?: string;
  smtp_pass?: string;
  smtp_use_tls?: boolean;
  smtp_use_starttls?: boolean;
  is_default: boolean;
  validate_connection?: boolean;
}
