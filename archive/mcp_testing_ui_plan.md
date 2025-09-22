# RustyMail SSE Dashboard Frontend Implementation Plan

## 1. Introduction

This document provides a comprehensive implementation plan for the RustyMail SSE Dashboard frontend. It details the UI development approach, component architecture, API contract, and testing strategy to build a modern, responsive, and user-friendly dashboard with a Steve Jobs-inspired UX philosophy.

The frontend will be implemented as a standalone React application, capable of functioning with mock data during development and seamlessly integrating with the Rust backend when available.

## 2. Frontend Technology Stack

### Core Technologies
- **Framework**: React 18+ with Vite
- **Language**: TypeScript 5.0+
- **Styling**: Tailwind CSS 3.3+ with shadcn/ui components

### State Management & Data Fetching
- **API Client**: React Query v5 (TanStack Query)
- **Local State**: React Context + useReducer for complex state
- **URL State**: nuqs for searchable/shareable URL parameters

### UI Components
- **Forms**: React Hook Form + Conform + Zod for validation
- **Tables**: TanStack Table (React Table v8)
- **Charts**: Recharts for statistics visualization
- **Animations**: Framer Motion for transitions and micro-interactions

### Developer Experience
- **Code Quality**: ESLint + Prettier
- **Testing**: Playwright for E2E testing
- **Build**: Vite with optimized production settings

### Additional Libraries
- **Date Handling**: date-fns
- **AI Integration**: Vercel AI SDK for chatbot
- **User Onboarding**: NextStepJS for guided tours
- **Icon System**: Lucide React for consistent iconography

## 3. Project Structure

```
/src
  /api                   # API client, hooks, and mock implementations
    /mocks               # Mock data and API responses
  /components            # Reusable UI components
    /ui                  # shadcn/ui components
    /layout              # Layout components
    /dashboard           # Dashboard-specific components
    /stats               # Statistics components
    /client-list         # Client list components
    /chatbot             # AI Chatbot components
  /lib                   # Utilities and helpers
  /hooks                 # Custom React hooks
  /styles                # Global styles and Tailwind config
  /types                 # TypeScript type definitions
  /context               # React context providers
  /features              # Feature-specific components
  /pages                 # Page components
  App.tsx                # Main application component
  main.tsx               # Entry point
```

## 4. Steve Jobs-Inspired UX Principles

### Design Philosophy
- **Simplicity**: Eliminate unnecessary elements
- **Focus**: Guide attention to what matters most
- **Clarity**: Make functionality self-evident
- **Efficiency**: Optimize for common workflows
- **Elegance**: Subtle refinement in every detail

### Visual Design
- **Color**: Minimal palette with purposeful accent colors
- **Typography**: Clear hierarchical type system
- **Space**: Generous whitespace creating visual breathing room
- **Animation**: Subtle, purposeful motion enhancing understanding

### Interaction Design
- **Responsiveness**: Immediate feedback for all actions
- **Consistency**: Predictable patterns throughout the interface
- **Refinement**: Polish in every interaction
- **Accessibility**: Designed for universal usability

## 5. Component Specifications

### Layout Structure

```
+--------------------------------------------------------------+
|                         Top Bar                              |
+--------------------------------------------------------------+
|                                                              |
|  +------------------------+  +---------------------------+   |
|  |                        |  |                           |   |
|  |     Stats Panel        |  |      Client List          |   |
|  |                        |  |                           |   |
|  +------------------------+  +---------------------------+   |
|                                                              |
|  +------------------------------------------------------+   |
|  |                                                      |   |
|  |                  AI Chatbot Panel                    |   |
|  |                                                      |   |
|  +------------------------------------------------------+   |
|                                                              |
+--------------------------------------------------------------+
```

### Top Bar Component
```
+--------------------------------------------------------------+
| RustyMail Dashboard      |      IMAP Adapter: [Mock ▼]       |
+--------------------------------------------------------------+
```

### Stats Panel Component
```
+------------------------+
| Statistics             |
+------------------------+
| +--------------------+ |
| |  Connection Count  | |
| |        42         | |
| +--------------------+ |
|                        |
| +--------------------+ |
| |    Request Rate    | |
| |    [Chart Line]    | |
| +--------------------+ |
|                        |
| +--------------------+ |
| |   System Health    | |
| |     [Indicator]    | |
| +--------------------+ |
+------------------------+
```

### Client List Component
```
+-----------------------------------------------+
| Connected Clients (3)               [Filter ⌄] |
+-----------------------------------------------+
| ID       | Type    | Connected        | Status |
+---------+---------+-----------------+--------+
| client1  | SSE     | 10m ago         | Active |
+---------+---------+-----------------+--------+
| client2  | API     | 5m ago          | Active |
+---------+---------+-----------------+--------+
| client3  | Console | 1m ago          | Idle   |
+---------+---------+-----------------+--------+
|                                             |
|           [Pagination Controls]             |
+---------------------------------------------+
```

### AI Chatbot Component
```
+-----------------------------------------------+
| Email Assistant                               |
+-----------------------------------------------+
|                                               |
|  +-------------------------------------+      |
|  | You: How many emails in my inbox?   |      |
|  +-------------------------------------+      |
|                                               |
|  +-------------------------------------+      |
|  | AI: You have 12 unread emails in    |      |
|  | your inbox. Would you like me to    |      |
|  | list them for you?                  |      |
|  +-------------------------------------+      |
|                                               |
|  +-------------------------------------+      |
|  | You: Yes, show me the most recent 5 |      |
|  +-------------------------------------+      |
|                                               |
|  +-------------------------------------+      |
|  | AI: Here are your 5 most recent     |      |
|  | emails:                             |      |
|  | 1. John Doe - Meeting Notes (2m ago)|      |
|  | 2. Jane Smith - Project Update (1h) |      |
|  | ...                                 |      |
|  +-------------------------------------+      |
|                                               |
| +---------------------------------------+     |
| | Type your message...            [Send] |     |
| +---------------------------------------+     |
+-----------------------------------------------+
```

## 6. API Contract

### Endpoints

#### 1. `/api/dashboard/stats`
- **Method**: GET
- **Response**: Dashboard statistics data
- **Mock Implementation**: Return randomized stats with time-based trends

#### 2. `/api/dashboard/clients`
- **Method**: GET
- **Parameters**: ?page=1&limit=10&filter=active
- **Response**: List of connected clients with pagination
- **Mock Implementation**: Generate variable number of clients with different states

#### 3. `/api/dashboard/config`
- **Method**: GET
- **Response**: Current server configuration including active IMAP adapter
- **Mock Implementation**: Return predefined config with selectable adapters

#### 4. `/api/dashboard/chatbot/query`
- **Method**: POST
- **Body**: `{ "query": "text message from user", "conversation_id": "optional-id" }`
- **Response**: AI response with related email data if applicable
- **Mock Implementation**: Pattern-matched responses for common email queries

#### 5. SSE Events (EventSource)
- **Endpoint**: `/api/dashboard/events`
- **Event Types**: 
  - `stats_updated`: Real-time statistics updates
  - `client_connected`: New client connection
  - `client_disconnected`: Client disconnection
  - `system_alert`: Important system notifications
- **Mock Implementation**: Emit events on timers to simulate real-time updates

### Type Definitions (TypeScript)

```typescript
// Stats Types
interface DashboardStats {
  activeConnections: number;
  requestRate: RequestRateData[];
  systemHealth: SystemHealth;
  lastUpdated: string; // ISO timestamp
}

interface RequestRateData {
  timestamp: string;
  value: number;
}

interface SystemHealth {
  status: 'healthy' | 'degraded' | 'critical';
  memoryUsage: number; // percentage
  cpuUsage: number; // percentage
}

// Client Types
interface ClientInfo {
  id: string;
  type: 'SSE' | 'API' | 'Console';
  connectedAt: string; // ISO timestamp
  status: 'Active' | 'Idle' | 'Disconnecting';
  lastActivity: string; // ISO timestamp
  ipAddress?: string;
  userAgent?: string;
}

interface ClientListResponse {
  clients: ClientInfo[];
  pagination: {
    total: number;
    page: number;
    limit: number;
    totalPages: number;
  }
}

// Config Types
interface ServerConfig {
  activeAdapter: ImapAdapter;
  availableAdapters: ImapAdapter[];
  version: string;
  uptime: number; // seconds
}

interface ImapAdapter {
  id: string;
  name: string;
  description: string;
  isActive: boolean;
}

// Chatbot Types
interface ChatbotQuery {
  query: string;
  conversation_id?: string;
}

interface ChatbotResponse {
  text: string;
  conversation_id: string;
  emailData?: EmailData;
  followupSuggestions?: string[];
}

interface EmailData {
  // Various email-related data returned by chatbot
  messages?: EmailMessage[];
  count?: number;
  folders?: EmailFolder[];
}

interface EmailMessage {
  id: string;
  subject: string;
  from: string;
  date: string;
  snippet: string;
  isRead: boolean;
}

interface EmailFolder {
  name: string;
  count: number;
  unreadCount: number;
}
```

## 7. Mock Data Implementation

### Mock Data Approach
1. Create realistic mock data factories for all API responses
2. Implement variable response times (200-800ms) to simulate network latency
3. Add occasional simulated errors (5% of requests) for error handling testing
4. Store conversation history in localStorage for chatbot context persistence
5. Implement SSE event simulation with configurable frequency

### Mock Data Examples

```typescript
// Example Stats Mock Data Generator
function generateMockStats(): DashboardStats {
  const now = new Date();
  
  return {
    activeConnections: Math.floor(Math.random() * 50) + 5,
    requestRate: Array.from({ length: 24 }).map((_, i) => ({
      timestamp: new Date(now.getTime() - (23 - i) * 5 * 60000).toISOString(),
      value: Math.floor(Math.random() * 100) + 20
    })),
    systemHealth: {
      status: Math.random() > 0.9 ? 'degraded' : 'healthy',
      memoryUsage: Math.floor(Math.random() * 60) + 20,
      cpuUsage: Math.floor(Math.random() * 40) + 10
    },
    lastUpdated: now.toISOString()
  };
}

// Example Client List Mock Data
function generateMockClients(page: number, limit: number): ClientListResponse {
  const total = 47; // Total fake clients
  const clients = Array.from({ length: Math.min(limit, total - (page - 1) * limit) })
    .map((_, i) => {
      const id = `client-${(page - 1) * limit + i + 1}`;
      const types = ['SSE', 'API', 'Console'] as const;
      const statuses = ['Active', 'Idle', 'Disconnecting'] as const;
      
      return {
        id,
        type: types[Math.floor(Math.random() * types.length)],
        connectedAt: new Date(Date.now() - Math.random() * 86400000).toISOString(),
        status: statuses[Math.floor(Math.random() * statuses.length)],
        lastActivity: new Date(Date.now() - Math.random() * 3600000).toISOString(),
        ipAddress: `192.168.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
      };
    });
  
  return {
    clients,
    pagination: {
      total,
      page,
      limit,
      totalPages: Math.ceil(total / limit)
    }
  };
}
```

## 8. React Query Implementation

```typescript
// Example React Query Hooks

// Stats Hook
export function useStats() {
  return useQuery({
    queryKey: ['stats'],
    queryFn: async () => {
      const response = await fetch('/api/dashboard/stats');
      if (!response.ok) throw new Error('Failed to fetch stats');
      return response.json() as Promise<DashboardStats>;
    },
    refetchInterval: 30000, // Refetch every 30 seconds
  });
}

// Clients Hook with Pagination
export function useClients(page: number, limit: number, filter?: string) {
  return useQuery({
    queryKey: ['clients', page, limit, filter],
    queryFn: async () => {
      const params = new URLSearchParams({ 
        page: page.toString(), 
        limit: limit.toString() 
      });
      if (filter) params.append('filter', filter);
      
      const response = await fetch(`/api/dashboard/clients?${params}`);
      if (!response.ok) throw new Error('Failed to fetch clients');
      return response.json() as Promise<ClientListResponse>;
    },
    keepPreviousData: true, // Keep previous page data while loading next page
  });
}

// Chatbot Query Hook
export function useChatbotMutation() {
  return useMutation({
    mutationFn: async (query: ChatbotQuery) => {
      const response = await fetch('/api/dashboard/chatbot/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(query),
      });
      
      if (!response.ok) throw new Error('Failed to send query to chatbot');
      return response.json() as Promise<ChatbotResponse>;
    },
  });
}
```

## 9. SSE Integration

```typescript
// SSE Hook for Real-time Updates
export function useSSEEvents() {
  const [events, setEvents] = useState<SSEEvent[]>([]);
  const queryClient = useQueryClient();
  
  useEffect(() => {
    const eventSource = new EventSource('/api/dashboard/events');
    
    eventSource.addEventListener('stats_updated', (event) => {
      const data = JSON.parse(event.data);
      queryClient.setQueryData(['stats'], data);
      setEvents(prev => [...prev, { type: 'stats_updated', timestamp: new Date(), data }]);
    });
    
    eventSource.addEventListener('client_connected', (event) => {
      const data = JSON.parse(event.data);
      queryClient.invalidateQueries(['clients']);
      setEvents(prev => [...prev, { type: 'client_connected', timestamp: new Date(), data }]);
    });
    
    eventSource.addEventListener('client_disconnected', (event) => {
      const data = JSON.parse(event.data);
      queryClient.invalidateQueries(['clients']);
      setEvents(prev => [...prev, { type: 'client_disconnected', timestamp: new Date(), data }]);
    });
    
    eventSource.addEventListener('system_alert', (event) => {
      const data = JSON.parse(event.data);
      setEvents(prev => [...prev, { type: 'system_alert', timestamp: new Date(), data }]);
      // Could also trigger a toast notification here
    });
    
    return () => {
      eventSource.close();
    };
  }, [queryClient]);
  
  return events;
}
```

## 10. Component Implementation Plan

### Phase 1: Core Structure & Layout
- Project setup with Vite and TypeScript
- Installation of all dependencies
- Layout implementation with responsive design
- Theme setup with Tailwind configuration
- Basic routing and navigation

### Phase 2: Data & API Layer
- Mock API implementation for all endpoints
- React Query setup and fetch utilities
- SSE event simulation
- Local storage utilities for persistence
- Type definitions for all data structures

### Phase 3: UI Components
- Stats Panel with charts and metrics
- Client List with filtering, sorting, and pagination
- IMAP Adapter selector with persistence
- AI Chatbot interface with conversation history
- Toast notifications for system events

### Phase 4: Polish & Refinement
- Animation and transition implementation
- Accessibility improvements (ARIA, keyboard navigation)
- Performance optimizations
- Error handling and recovery
- Dark mode support

### Phase 5: Testing & Documentation
- Unit tests for utility functions
- Component tests with React Testing Library
- E2E tests with Playwright
- Storybook documentation for components
- User documentation

## 11. Gherkin Test Scenarios

```gherkin
Feature: Dashboard Layout and Basic Functionality

  Background:
    Given the user navigates to the dashboard page

  Scenario: Dashboard loads with correct title and layout
    Then the page title should be "RustyMail SSE Dashboard"
    And the dashboard should have a top bar with title
    And the dashboard should have a stats panel
    And the dashboard should have a client list panel
    And the dashboard should have an AI chatbot panel

  Scenario: Top bar shows IMAP adapter selector
    Then the top bar should display the IMAP adapter selector
    And the IMAP adapter selector should show "Mock" as default

  Scenario: IMAP adapter selection persists after page reload
    When the user selects "GoDaddy" in the IMAP adapter selector
    And the user reloads the page
    Then the IMAP adapter selector should show "GoDaddy"

Feature: Statistics Panel

  Background:
    Given the user navigates to the dashboard page

  Scenario: Stats panel shows connection count
    Then the stats panel should display current connection count
    And the connection count should be a number

  Scenario: Stats panel shows request rate chart
    Then the stats panel should display a request rate chart
    And the chart should have multiple data points

  Scenario: Stats panel updates in real-time
    When a stats_updated SSE event occurs
    Then the stats panel should update with new data without page reload
    And the update should have a smooth animation

Feature: Client List Panel

  Background:
    Given the user navigates to the dashboard page

  Scenario: Client list shows connected clients
    Then the client list should display the current clients
    And each client should show its ID, type, connection time, and status

  Scenario: Client list supports pagination
    Given there are more than 10 clients
    Then the client list should show pagination controls
    When the user clicks on the next page
    Then the client list should load the next set of clients

  Scenario: Client list supports filtering
    When the user selects "Active" from the filter dropdown
    Then the client list should only show active clients

  Scenario: Client list updates when client connects
    When a client_connected SSE event occurs
    Then the client list should add the new client
    And the new client row should have an entrance animation

Feature: AI Chatbot Interface

  Background:
    Given the user navigates to the dashboard page

  Scenario: Chatbot initial state
    Then the chatbot panel should show an empty conversation
    And the chatbot panel should have an input field
    And the chatbot panel should have a send button

  Scenario: Sending a message to the chatbot
    When the user types "How many emails are in my inbox?" in the chatbot input
    And the user clicks the send button
    Then the user message should appear in the conversation
    And the AI should respond with a message containing email count
    And the AI response should have a typing animation

  Scenario: Chatbot maintains conversation context
    When the user sends "Find emails from example@email.com"
    And the chatbot responds
    And the user sends "Which one is the most recent?"
    Then the chatbot should respond referring to emails from example@email.com

  Scenario: Chatbot handles errors gracefully
    When the API returns an error for a chatbot query
    Then the chatbot should display an error message
    And the chatbot should offer to retry the query

Feature: Accessibility and Keyboard Navigation

  Background:
    Given the user navigates to the dashboard page

  Scenario: Tab navigation works correctly
    When the user presses the Tab key repeatedly
    Then focus should move through all interactive elements in a logical order

  Scenario: Screen reader compatibility
    Then all important UI elements should have appropriate ARIA attributes
    And all images should have alt text

  Scenario: High contrast mode compatibility
    When the user has high contrast mode enabled
    Then the dashboard should remain readable and functional

Feature: Mobile Responsiveness

  Background:
    Given the user navigates to the dashboard page on a mobile device

  Scenario: Layout adapts to small screens
    Then the panels should stack vertically
    And all content should be readable without horizontal scrolling

  Scenario: Touch interactions work correctly
    When the user taps on interactive elements
    Then they should respond appropriately
    And touch targets should be large enough for comfortable use
```

## 12. Playwright Test Implementation

```typescript
// Example Playwright Test for Dashboard Layout

import { test, expect } from '@playwright/test';

test.describe('Dashboard Layout', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to dashboard and wait for it to load
    await page.goto('/');
    await page.waitForSelector('[data-testid="dashboard-layout"]');
  });

  test('should load with correct title and layout', async ({ page }) => {
    // Check title
    await expect(page).toHaveTitle('RustyMail SSE Dashboard');
    
    // Check main layout components
    await expect(page.locator('[data-testid="top-bar"]')).toBeVisible();
    await expect(page.locator('[data-testid="stats-panel"]')).toBeVisible();
    await expect(page.locator('[data-testid="client-list-panel"]')).toBeVisible();
    await expect(page.locator('[data-testid="chatbot-panel"]')).toBeVisible();
  });

  test('IMAP adapter selector shows default value', async ({ page }) => {
    const selector = page.locator('[data-testid="imap-adapter-selector"]');
    await expect(selector).toBeVisible();
    await expect(selector).toContainText('Mock');
  });

  test('IMAP adapter selection persists after page reload', async ({ page }) => {
    // Select a different adapter
    await page.locator('[data-testid="imap-adapter-selector"]').click();
    await page.locator('text=GoDaddy').click();
    
    // Verify selection
    await expect(page.locator('[data-testid="imap-adapter-selector"]')).toContainText('GoDaddy');
    
    // Reload page
    await page.reload();
    await page.waitForSelector('[data-testid="dashboard-layout"]');
    
    // Verify selection persists
    await expect(page.locator('[data-testid="imap-adapter-selector"]')).toContainText('GoDaddy');
  });
});

// Example test for the Chatbot

test.describe('AI Chatbot', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('[data-testid="chatbot-panel"]');
  });

  test('should send and receive messages', async ({ page }) => {
    // Type a message
    await page.locator('[data-testid="chatbot-input"]').fill('How many emails are in my inbox?');
    await page.locator('[data-testid="chatbot-send-button"]').click();
    
    // Check that user message appears
    await expect(page.locator('[data-testid="chatbot-messages"]'))
      .toContainText('How many emails are in my inbox?');
    
    // Wait for AI response and check that it contains an email count
    await expect(page.locator('[data-testid="chatbot-message-ai"]:last-child'))
      .toContainText(/\d+ (?:email|emails|message|messages)/);
  });

  test('should maintain conversation context', async ({ page }) => {
    // First message
    await page.locator('[data-testid="chatbot-input"]').fill('Find emails from example@email.com');
    await page.locator('[data-testid="chatbot-send-button"]').click();
    
    // Wait for first response
    await expect(page.locator('[data-testid="chatbot-message-ai"]:last-child'))
      .toContainText('example@email.com');
    
    // Follow-up question
    await page.locator('[data-testid="chatbot-input"]').fill('Which one is the most recent?');
    await page.locator('[data-testid="chatbot-send-button"]').click();
    
    // Check the AI maintains context in its response
    await expect(page.locator('[data-testid="chatbot-message-ai"]:last-child'))
      .toContainText(/recent.*example@email\.com/);
  });
});
```

## 13. Implementation Schedule

### Week 1: Foundation
- Setup project with Vite, TypeScript, and dependencies
- Implement basic layout structure
- Create API client and mock data services
- Setup state management and React Query

### Week 2: Core Components
- Implement Stats Panel with charts
- Build Client List with pagination
- Create IMAP Adapter selector with persistence
- Develop basic SSE event handling

### Week 3: Chatbot Implementation
- Build Chatbot UI with message history
- Implement mock conversation logic
- Create conversation persistence
- Design typing indicators and animations

### Week 4: Polish and Testing
- Add animations and transitions
- Implement responsive design adaptations
- Write Playwright tests
- Fix bugs and edge cases
- Performance optimization

## 14. Conclusion

This implementation plan provides a comprehensive roadmap for building the RustyMail SSE Dashboard frontend. By following the Steve Jobs-inspired UX principles and leveraging the specified technology stack, the result will be an elegant, intuitive, and powerful interface that provides users with both deep functionality and a delightful experience.

The mock data approach ensures development can proceed independently of the backend, while the detailed API contract ensures smooth integration when the Rust backend becomes available.
