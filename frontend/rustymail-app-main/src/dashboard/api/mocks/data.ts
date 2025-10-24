// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.


import { 
  DashboardStats, 
  ClientListResponse, 
  ServerConfig, 
  ChatbotResponse,
  EmailMessage,
  EmailFolder
} from '@/types';

// Generate mock stats data
export function generateMockStats(): DashboardStats {
  const now = new Date();
  
  return {
    activeConnections: Math.floor(Math.random() * 50) + 5,
    requestRate: Array.from({ length: 24 }).map((_, i) => ({
      timestamp: new Date(now.getTime() - (23 - i) * 5 * 60000).toISOString(),
      value: Math.floor(Math.random() * 100) + 20
    })),
    systemHealth: {
      status: Math.random() > 0.9 ? (Math.random() > 0.5 ? 'degraded' : 'critical') : 'healthy',
      memoryUsage: Math.floor(Math.random() * 60) + 20,
      cpuUsage: Math.floor(Math.random() * 40) + 10
    },
    lastUpdated: now.toISOString()
  };
}

// Generate mock client list data
export function generateMockClients(page: number, limit: number, filter?: string): ClientListResponse {
  const total = 47; // Total fake clients
  let clients = Array.from({ length: Math.min(limit, total - (page - 1) * limit) })
    .map((_, i) => {
      const id = `client-${(page - 1) * limit + i + 1}`;
      const types = ['SSE', 'API', 'Console'] as const;
      const statuses = ['Active', 'Idle', 'Disconnecting'] as const;
      const type = types[Math.floor(Math.random() * types.length)];
      const status = statuses[Math.floor(Math.random() * statuses.length)];
      
      return {
        id,
        type,
        connectedAt: new Date(Date.now() - Math.random() * 86400000).toISOString(),
        status,
        lastActivity: new Date(Date.now() - Math.random() * 3600000).toISOString(),
        ipAddress: `192.168.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
      };
    });
  
  // Apply filtering if requested
  if (filter) {
    clients = clients.filter(client => 
      client.status.toLowerCase() === filter.toLowerCase() ||
      client.type.toLowerCase() === filter.toLowerCase()
    );
  }
  
  return {
    clients,
    pagination: {
      total: filter ? clients.length : total,
      page,
      limit,
      totalPages: Math.ceil((filter ? clients.length : total) / limit)
    }
  };
}

// Generate mock server config
export function generateMockConfig(): ServerConfig {
  return {
    activeAdapter: {
      id: 'mock',
      name: 'Mock',
      description: 'Mock IMAP adapter for development',
      isActive: true
    },
    availableAdapters: [
      {
        id: 'mock',
        name: 'Mock',
        description: 'Mock IMAP adapter for development',
        isActive: true
      },
      {
        id: 'gmail',
        name: 'Gmail',
        description: 'Google Gmail IMAP adapter',
        isActive: false
      },
      {
        id: 'outlook',
        name: 'Outlook',
        description: 'Microsoft Outlook IMAP adapter',
        isActive: false
      },
      {
        id: 'yahoo',
        name: 'Yahoo',
        description: 'Yahoo Mail IMAP adapter',
        isActive: false
      },
      {
        id: 'godaddy',
        name: 'GoDaddy',
        description: 'GoDaddy IMAP adapter',
        isActive: false
      }
    ],
    version: '1.0.0',
    uptime: Math.floor(Math.random() * 1000000)
  };
}

// Sample email templates for the chatbot
const sampleEmails: EmailMessage[] = [
  {
    id: 'e1',
    subject: 'Project Update - Q2 Roadmap',
    from: 'john.doe@example.com',
    date: new Date(Date.now() - 30 * 60000).toISOString(),
    snippet: 'I wanted to share the latest updates on our Q2 roadmap. We have made significant progress on...',
    isRead: false
  },
  {
    id: 'e2',
    subject: 'Meeting Notes: Marketing Strategy',
    from: 'jane.smith@example.com',
    date: new Date(Date.now() - 120 * 60000).toISOString(),
    snippet: 'Attached are the notes from our marketing strategy meeting yesterday. Key takeaways include...',
    isRead: true
  },
  {
    id: 'e3',
    subject: 'Invoice #12345',
    from: 'billing@acmeservices.com',
    date: new Date(Date.now() - 5 * 3600000).toISOString(),
    snippet: 'Please find attached your invoice #12345 for services rendered in April 2023...',
    isRead: false
  },
  {
    id: 'e4',
    subject: 'New Feature Announcement',
    from: 'product@ourcompany.com',
    date: new Date(Date.now() - 12 * 3600000).toISOString(),
    snippet: 'We\'re excited to announce the launch of our new feature that will revolutionize how you...',
    isRead: false
  },
  {
    id: 'e5',
    subject: 'Weekly Team Digest',
    from: 'team-updates@example.org',
    date: new Date(Date.now() - 24 * 3600000).toISOString(),
    snippet: 'This week\'s highlights: • New client onboarding • Product launch success • Team lunch on Friday...',
    isRead: true
  },
  {
    id: 'e6',
    subject: 'Your Subscription Renewal',
    from: 'support@saasproduct.com',
    date: new Date(Date.now() - 48 * 3600000).toISOString(),
    snippet: 'Your premium subscription is due for renewal on May 1st. To continue enjoying uninterrupted access...',
    isRead: true
  },
  {
    id: 'e7',
    subject: 'Security Alert',
    from: 'security@yourbank.com',
    date: new Date(Date.now() - 70 * 3600000).toISOString(),
    snippet: 'We detected a login attempt from a new device. If this was you, no action is needed. If not...',
    isRead: false
  }
];

// Sample email folders
const emailFolders: EmailFolder[] = [
  { name: 'Inbox', count: 24, unreadCount: 12 },
  { name: 'Sent', count: 103, unreadCount: 0 },
  { name: 'Drafts', count: 5, unreadCount: 5 },
  { name: 'Spam', count: 17, unreadCount: 17 },
  { name: 'Trash', count: 43, unreadCount: 0 },
  { name: 'Work', count: 78, unreadCount: 8 },
  { name: 'Personal', count: 32, unreadCount: 4 }
];

// Handles chatbot queries and returns appropriate responses
export function handleChatbotQuery(query: string, conversationId?: string): ChatbotResponse {
  // Simple NLP patterns for demo purposes
  const lowerQuery = query.toLowerCase();
  let response = '';
  let emailData: any = undefined;
  let followupSuggestions: string[] = [];

  // Detect email count queries
  if (lowerQuery.includes('how many') && 
      (lowerQuery.includes('email') || lowerQuery.includes('message') || lowerQuery.includes('inbox'))) {
    response = `You have 12 unread emails in your inbox out of 24 total messages.`;
    emailData = { count: 24, unreadCount: 12 };
    followupSuggestions = ['Show me unread emails', 'Read the most recent email', 'Any important emails?'];
  } 
  // Detect mail listing queries
  else if ((lowerQuery.includes('show') || lowerQuery.includes('list') || lowerQuery.includes('get')) && 
          (lowerQuery.includes('email') || lowerQuery.includes('message'))) {
    const count = lowerQuery.includes('unread') ? 5 : 
                 (lowerQuery.match(/(\d+)/g) ? parseInt(lowerQuery.match(/(\d+)/g)![0]) : 5);
    
    response = `Here are your ${lowerQuery.includes('unread') ? 'unread' : ''} ${count} most recent emails:`;
    emailData = { 
      messages: sampleEmails
        .filter(email => lowerQuery.includes('unread') ? !email.isRead : true)
        .slice(0, count) 
    };
    followupSuggestions = ['Mark all as read', 'Show me more emails', 'Any emails from John?'];
  }
  // Detect sender-specific queries
  else if (lowerQuery.includes('from') && lowerQuery.includes('@')) {
    const emailPattern = /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b/;
    const matches = query.match(emailPattern);
    const sender = matches ? matches[0] : 'unknown';
    
    const filteredEmails = sampleEmails.filter(email => 
      email.from.toLowerCase().includes(sender.toLowerCase())
    );
    
    if (filteredEmails.length > 0) {
      response = `I found ${filteredEmails.length} email${filteredEmails.length > 1 ? 's' : ''} from ${sender}:`;
      emailData = { messages: filteredEmails };
      followupSuggestions = ['Which one is the most recent?', 'Are any unread?', 'Summarize these emails'];
    } else {
      response = `I couldn't find any emails from ${sender}.`;
      followupSuggestions = ['Check spam folder', 'Search for emails about project updates', 'Show recent emails'];
    }
  }
  // Detect folder queries
  else if (lowerQuery.includes('folder') || 
          (['inbox', 'sent', 'draft', 'spam', 'trash'].some(folder => lowerQuery.includes(folder)))) {
    const folderNames = ['inbox', 'sent', 'draft', 'spam', 'trash', 'work', 'personal'];
    const mentionedFolder = folderNames.find(folder => lowerQuery.includes(folder));
    
    if (mentionedFolder) {
      const folder = emailFolders.find(f => f.name.toLowerCase() === mentionedFolder);
      response = `Your ${mentionedFolder} folder has ${folder?.count} emails, ${folder?.unreadCount} unread.`;
      emailData = { folders: [folder] };
      followupSuggestions = ['Show me these emails', 'Any recent emails?', 'Check other folders'];
    } else {
      response = `Here are your email folders:`;
      emailData = { folders: emailFolders };
      followupSuggestions = ['Show me inbox emails', 'How many unread in spam?', 'Check work folder'];
    }
  }
  // Follow-ups for "most recent" when in context of previous emails
  else if (conversationId && 
          (lowerQuery.includes('recent') || lowerQuery.includes('latest') || lowerQuery.includes('newest'))) {
    // In a real app, we'd look up the conversationId to get context
    // For demo, we'll just return the most recent email
    response = `The most recent email is from ${sampleEmails[0].from} with subject "${sampleEmails[0].subject}", received ${formatTimeAgo(new Date(sampleEmails[0].date))}.`;
    emailData = { messages: [sampleEmails[0]] };
    followupSuggestions = ['Read this email', 'Reply to this email', 'Are there other recent emails?'];
  }
  // Generic fallback
  else {
    response = `I'm your email assistant. You can ask me about your emails, show recent messages, check specific folders, or find emails from particular senders.`;
    followupSuggestions = ['How many unread emails do I have?', 'Show me recent emails', 'Check my inbox'];
  }

  return {
    text: response,
    conversation_id: conversationId || `conv_${Date.now()}`,
    emailData,
    followupSuggestions
  };
}

// Helper function to format time ago
function formatTimeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHours = Math.floor(diffMin / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSec < 60) return `${diffSec} second${diffSec !== 1 ? 's' : ''} ago`;
  if (diffMin < 60) return `${diffMin} minute${diffMin !== 1 ? 's' : ''} ago`;
  if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
  return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;
}
