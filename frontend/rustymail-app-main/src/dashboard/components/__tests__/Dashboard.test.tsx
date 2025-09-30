import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { Dashboard } from '../Dashboard';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';

// Mock child components
vi.mock('../TopBar', () => ({
  TopBar: () => <div data-testid="top-bar">TopBar</div>
}));

vi.mock('../StatsPanel', () => ({
  StatsPanel: () => <div data-testid="stats-panel">StatsPanel</div>
}));

vi.mock('../ClientListPanel', () => ({
  ClientListPanel: () => <div data-testid="client-list-panel">ClientListPanel</div>
}));

vi.mock('../ChatbotPanel', () => ({
  ChatbotPanel: () => <div data-testid="chatbot-panel">ChatbotPanel</div>
}));

vi.mock('../McpTools', () => ({
  McpTools: () => <div data-testid="mcp-tools">McpTools</div>
}));

// Mock SSE hook
vi.mock('../../hooks/useSSEEvents', () => ({
  useSSEEvents: () => ({
    isConnected: true,
    lastEvent: null,
    error: null,
  })
}));

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
};

describe('Dashboard Component', () => {
  it('renders all dashboard sections', () => {
    render(<Dashboard />, { wrapper: createWrapper() });

    expect(screen.getByTestId('top-bar')).toBeInTheDocument();
    expect(screen.getByTestId('stats-panel')).toBeInTheDocument();
    expect(screen.getByTestId('client-list-panel')).toBeInTheDocument();
    expect(screen.getByTestId('chatbot-panel')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-tools')).toBeInTheDocument();
  });

  it('displays loading state initially', () => {
    render(<Dashboard />, { wrapper: createWrapper() });

    // Check for loading indicators if any
    expect(screen.getByText(/RustyMail Dashboard/i)).toBeInTheDocument();
  });

  it('handles responsive layout', () => {
    const { container } = render(<Dashboard />, { wrapper: createWrapper() });

    // Check grid layout classes
    const mainGrid = container.querySelector('.grid');
    expect(mainGrid).toHaveClass('gap-4');
  });

  it('maintains proper component hierarchy', () => {
    const { container } = render(<Dashboard />, { wrapper: createWrapper() });

    const dashboard = container.firstChild;
    expect(dashboard).toHaveClass('min-h-screen');
    expect(dashboard).toHaveClass('bg-gray-50');
  });
});