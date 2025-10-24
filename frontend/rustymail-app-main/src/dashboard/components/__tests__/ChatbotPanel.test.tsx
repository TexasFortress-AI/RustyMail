// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import '@testing-library/jest-dom';
import { ChatbotPanel } from '../ChatbotPanel';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';

// Mock the API hooks
vi.mock('../../api/hooks', () => ({
  useChatbot: vi.fn(() => ({
    mutate: vi.fn(),
    isLoading: false,
    error: null,
  })),
  useStreamChatbot: vi.fn(() => ({
    mutate: vi.fn(),
    isStreaming: false,
    streamedResponse: '',
    error: null,
  })),
}));

// Mock SSE hook
vi.mock('../../api/useSSEChatbot', () => ({
  useSSEChatbot: () => ({
    sendMessage: vi.fn(),
    messages: [],
    isConnected: true,
    isStreaming: false,
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

describe('ChatbotPanel Component', () => {
  it('renders chatbot interface', () => {
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    expect(screen.getByText('AI Assistant')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Ask about your emails/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Send/i })).toBeInTheDocument();
  });

  it('handles user input', async () => {
    const user = userEvent.setup();
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    const input = screen.getByPlaceholderText(/Ask about your emails/i);
    await user.type(input, 'Show me unread emails');

    expect(input).toHaveValue('Show me unread emails');
  });

  it('sends message on button click', async () => {
    const { useChatbot } = require('../../api/hooks');
    const mockMutate = vi.fn();
    useChatbot.mockReturnValue({
      mutate: mockMutate,
      isLoading: false,
      error: null,
    });

    const user = userEvent.setup();
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    const input = screen.getByPlaceholderText(/Ask about your emails/i);
    const sendButton = screen.getByRole('button', { name: /Send/i });

    await user.type(input, 'Test message');
    await user.click(sendButton);

    expect(mockMutate).toHaveBeenCalledWith({
      query: 'Test message',
      context: expect.any(Object),
    });
  });

  it('sends message on Enter key press', async () => {
    const { useChatbot } = require('../../api/hooks');
    const mockMutate = vi.fn();
    useChatbot.mockReturnValue({
      mutate: mockMutate,
      isLoading: false,
      error: null,
    });

    const user = userEvent.setup();
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    const input = screen.getByPlaceholderText(/Ask about your emails/i);

    await user.type(input, 'Test message{Enter}');

    expect(mockMutate).toHaveBeenCalledWith({
      query: 'Test message',
      context: expect.any(Object),
    });
  });

  it('disables input while loading', () => {
    const { useChatbot } = require('../../api/hooks');
    useChatbot.mockReturnValue({
      mutate: vi.fn(),
      isLoading: true,
      error: null,
    });

    render(<ChatbotPanel />, { wrapper: createWrapper() });

    const input = screen.getByPlaceholderText(/Ask about your emails/i);
    const sendButton = screen.getByRole('button', { name: /Send/i });

    expect(input).toBeDisabled();
    expect(sendButton).toBeDisabled();
    expect(screen.getByText(/Processing.../i)).toBeInTheDocument();
  });

  it('displays error messages', () => {
    const { useChatbot } = require('../../api/hooks');
    useChatbot.mockReturnValue({
      mutate: vi.fn(),
      isLoading: false,
      error: new Error('Failed to send message'),
    });

    render(<ChatbotPanel />, { wrapper: createWrapper() });

    expect(screen.getByText(/Failed to send message/i)).toBeInTheDocument();
  });

  it('displays chat history', () => {
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    // Mock some chat history
    const chatHistory = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi! How can I help you?' },
    ];

    // Re-render with history
    const { rerender } = render(<ChatbotPanel initialHistory={chatHistory} />, { wrapper: createWrapper() });

    expect(screen.getByText('Hello')).toBeInTheDocument();
    expect(screen.getByText('Hi! How can I help you?')).toBeInTheDocument();
  });

  it('shows streaming response', async () => {
    const { useStreamChatbot } = require('../../api/hooks');
    useStreamChatbot.mockReturnValue({
      mutate: vi.fn(),
      isStreaming: true,
      streamedResponse: 'This is a streaming',
      error: null,
    });

    render(<ChatbotPanel />, { wrapper: createWrapper() });

    expect(screen.getByText(/This is a streaming/i)).toBeInTheDocument();
    expect(screen.getByText(/●●●/)).toBeInTheDocument(); // Streaming indicator
  });

  it('clears input after sending', async () => {
    const user = userEvent.setup();
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    const input = screen.getByPlaceholderText(/Ask about your emails/i) as HTMLInputElement;
    const sendButton = screen.getByRole('button', { name: /Send/i });

    await user.type(input, 'Test message');
    expect(input.value).toBe('Test message');

    await user.click(sendButton);
    expect(input.value).toBe('');
  });

  it('handles predefined prompts', async () => {
    const { useChatbot } = require('../../api/hooks');
    const mockMutate = vi.fn();
    useChatbot.mockReturnValue({
      mutate: mockMutate,
      isLoading: false,
      error: null,
    });

    const user = userEvent.setup();
    render(<ChatbotPanel />, { wrapper: createWrapper() });

    // Click a predefined prompt button if available
    const promptButton = screen.queryByText(/Show unread emails/i);
    if (promptButton) {
      await user.click(promptButton);
      expect(mockMutate).toHaveBeenCalled();
    }
  });

  it('handles SSE connection status', () => {
    const { useSSEChatbot } = require('../../api/useSSEChatbot');
    useSSEChatbot.mockReturnValue({
      sendMessage: vi.fn(),
      messages: [],
      isConnected: false,
      isStreaming: false,
      error: 'Connection lost',
    });

    render(<ChatbotPanel />, { wrapper: createWrapper() });

    expect(screen.getByText(/Connection lost/i)).toBeInTheDocument();
  });

  it('scrolls to bottom on new messages', async () => {
    const scrollIntoViewMock = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoViewMock;

    render(<ChatbotPanel />, { wrapper: createWrapper() });

    // Simulate new message
    const { rerender } = render(
      <ChatbotPanel initialHistory={[{ role: 'user', content: 'New message' }]} />,
      { wrapper: createWrapper() }
    );

    await waitFor(() => {
      expect(scrollIntoViewMock).toHaveBeenCalled();
    });
  });
});