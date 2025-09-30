import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { StatsPanel } from '../StatsPanel';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';

// Mock the API client
vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
  }
}));

// Mock the useStats hook
vi.mock('../../api/hooks', () => ({
  useStats: vi.fn(() => ({
    data: {
      total_messages: 1234,
      active_connections: 5,
      total_folders: 12,
      unread_messages: 42,
      recent_activity: 15,
      cache_hit_rate: 0.85,
    },
    isLoading: false,
    error: null,
  }))
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

describe('StatsPanel Component', () => {
  it('renders stats panel with title', () => {
    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('Dashboard Statistics')).toBeInTheDocument();
  });

  it('displays all stat cards', () => {
    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('Total Messages')).toBeInTheDocument();
    expect(screen.getByText('1,234')).toBeInTheDocument();

    expect(screen.getByText('Active Connections')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();

    expect(screen.getByText('Total Folders')).toBeInTheDocument();
    expect(screen.getByText('12')).toBeInTheDocument();

    expect(screen.getByText('Unread Messages')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    const { useStats } = require('../../api/hooks');
    useStats.mockReturnValue({
      data: null,
      isLoading: true,
      error: null,
    });

    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('Loading statistics...')).toBeInTheDocument();
  });

  it('displays error state', () => {
    const { useStats } = require('../../api/hooks');
    useStats.mockReturnValue({
      data: null,
      isLoading: false,
      error: new Error('Failed to fetch stats'),
    });

    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText(/Failed to load statistics/i)).toBeInTheDocument();
  });

  it('formats large numbers correctly', () => {
    const { useStats } = require('../../api/hooks');
    useStats.mockReturnValue({
      data: {
        total_messages: 1234567,
        active_connections: 100,
        total_folders: 999,
        unread_messages: 10000,
      },
      isLoading: false,
      error: null,
    });

    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('1,234,567')).toBeInTheDocument();
    expect(screen.getByText('10,000')).toBeInTheDocument();
  });

  it('updates when SSE event is received', async () => {
    const { useStats } = require('../../api/hooks');
    const mockUseStats = vi.fn();
    mockUseStats.mockReturnValue({
      data: {
        total_messages: 100,
        active_connections: 1,
      },
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    });
    useStats.mockImplementation(mockUseStats);

    const { rerender } = render(<StatsPanel />, { wrapper: createWrapper() });

    // Simulate SSE update
    mockUseStats.mockReturnValue({
      data: {
        total_messages: 101,
        active_connections: 2,
      },
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    });

    rerender(<StatsPanel />);

    await waitFor(() => {
      expect(screen.getByText('101')).toBeInTheDocument();
      expect(screen.getByText('2')).toBeInTheDocument();
    });
  });

  it('handles empty data gracefully', () => {
    const { useStats } = require('../../api/hooks');
    useStats.mockReturnValue({
      data: {},
      isLoading: false,
      error: null,
    });

    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('0')).toBeInTheDocument();
  });

  it('displays cache hit rate as percentage', () => {
    const { useStats } = require('../../api/hooks');
    useStats.mockReturnValue({
      data: {
        cache_hit_rate: 0.856,
      },
      isLoading: false,
      error: null,
    });

    render(<StatsPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('Cache Hit Rate')).toBeInTheDocument();
    expect(screen.getByText('85.6%')).toBeInTheDocument();
  });
});