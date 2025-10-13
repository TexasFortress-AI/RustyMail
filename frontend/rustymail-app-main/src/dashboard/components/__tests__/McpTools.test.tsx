import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import '@testing-library/jest-dom';
import { McpTools } from '../McpTools';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';

// Mock the API hooks
vi.mock('../../api/hooks', () => ({
  useMcpTools: vi.fn(() => ({
    data: [
      {
        name: 'list_folders',
        description: 'List all email folders',
        parameters: {},
      },
      {
        name: 'search_emails',
        description: 'Search for emails',
        parameters: {
          folder: 'string',
          query: 'string',
          max_results: 'number',
        },
      },
      {
        name: 'atomic_move_message',
        description: 'Move a message',
        parameters: {
          source_folder: 'string',
          target_folder: 'string',
          uid: 'string',
        },
      },
      {
        name: 'create_folder',
        description: 'Create a new email folder in the account',
        parameters: {
          folder_name: 'string',
        },
      },
      {
        name: 'delete_folder',
        description: 'Delete an email folder from the account',
        parameters: {
          folder_name: 'string',
        },
      },
      {
        name: 'rename_folder',
        description: 'Rename an email folder in the account',
        parameters: {
          old_name: 'string',
          new_name: 'string',
        },
      },
    ],
    isLoading: false,
    error: null,
  })),
  useExecuteMcpTool: vi.fn(() => ({
    mutate: vi.fn(),
    isLoading: false,
    error: null,
    data: null,
  })),
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

describe('McpTools Component', () => {
  it('renders MCP Tools panel', () => {
    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText('MCP Email Tools')).toBeInTheDocument();
    expect(screen.getByText('Available Tools')).toBeInTheDocument();
  });

  it('displays list of available tools', () => {
    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText('list_folders')).toBeInTheDocument();
    expect(screen.getByText('List all email folders')).toBeInTheDocument();
    expect(screen.getByText('search_emails')).toBeInTheDocument();
    expect(screen.getByText('Search for emails')).toBeInTheDocument();
    expect(screen.getByText('atomic_move_message')).toBeInTheDocument();
    expect(screen.getByText('Move a message')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    const { useMcpTools } = require('../../api/hooks');
    useMcpTools.mockReturnValue({
      data: null,
      isLoading: true,
      error: null,
    });

    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText('Loading tools...')).toBeInTheDocument();
  });

  it('displays error state', () => {
    const { useMcpTools } = require('../../api/hooks');
    useMcpTools.mockReturnValue({
      data: null,
      isLoading: false,
      error: new Error('Failed to load tools'),
    });

    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText(/Failed to load tools/i)).toBeInTheDocument();
  });

  it('selects a tool when clicked', async () => {
    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    const toolButton = screen.getByText('list_folders');
    await user.click(toolButton);

    expect(toolButton.parentElement).toHaveClass('border-blue-500');
    expect(screen.getByText('Execute')).toBeInTheDocument();
  });

  it('displays parameter inputs for selected tool', async () => {
    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    const searchTool = screen.getByText('search_emails');
    await user.click(searchTool);

    expect(screen.getByLabelText('folder')).toBeInTheDocument();
    expect(screen.getByLabelText('query')).toBeInTheDocument();
    expect(screen.getByLabelText('max_results')).toBeInTheDocument();
  });

  it('handles parameter input', async () => {
    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    // Select search_emails tool
    await user.click(screen.getByText('search_emails'));

    // Fill in parameters
    const folderInput = screen.getByLabelText('folder') as HTMLInputElement;
    const queryInput = screen.getByLabelText('query') as HTMLInputElement;

    await user.type(folderInput, 'INBOX');
    await user.type(queryInput, 'FROM john@example.com');

    expect(folderInput.value).toBe('INBOX');
    expect(queryInput.value).toBe('FROM john@example.com');
  });

  it('executes selected tool with parameters', async () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    const mockMutate = vi.fn();
    useExecuteMcpTool.mockReturnValue({
      mutate: mockMutate,
      isLoading: false,
      error: null,
      data: null,
    });

    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    // Select and configure tool
    await user.click(screen.getByText('search_emails'));
    await user.type(screen.getByLabelText('folder'), 'INBOX');
    await user.type(screen.getByLabelText('query'), 'ALL');

    // Execute
    await user.click(screen.getByText('Execute'));

    expect(mockMutate).toHaveBeenCalledWith({
      tool: 'search_emails',
      parameters: {
        folder: 'INBOX',
        query: 'ALL',
        max_results: '',
      },
    });
  });

  it('displays execution results', () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    useExecuteMcpTool.mockReturnValue({
      mutate: vi.fn(),
      isLoading: false,
      error: null,
      data: {
        success: true,
        data: [
          { uid: '1', subject: 'Test Email 1' },
          { uid: '2', subject: 'Test Email 2' },
        ],
      },
    });

    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText(/Result:/i)).toBeInTheDocument();
    expect(screen.getByText(/Test Email 1/i)).toBeInTheDocument();
    expect(screen.getByText(/Test Email 2/i)).toBeInTheDocument();
  });

  it('shows execution loading state', () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    useExecuteMcpTool.mockReturnValue({
      mutate: vi.fn(),
      isLoading: true,
      error: null,
      data: null,
    });

    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText('Executing...')).toBeInTheDocument();
    expect(screen.getByText('Execute')).toBeDisabled();
  });

  it('displays execution error', () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    useExecuteMcpTool.mockReturnValue({
      mutate: vi.fn(),
      isLoading: false,
      error: new Error('Execution failed'),
      data: null,
    });

    render(<McpTools />, { wrapper: createWrapper() });

    expect(screen.getByText(/Execution failed/i)).toBeInTheDocument();
  });

  it('clears results when selecting a different tool', async () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    const executeMock = {
      mutate: vi.fn(),
      isLoading: false,
      error: null,
      data: { success: true, data: 'Previous result' },
    };
    useExecuteMcpTool.mockReturnValue(executeMock);

    const user = userEvent.setup();
    const { rerender } = render(<McpTools />, { wrapper: createWrapper() });

    // Select first tool
    await user.click(screen.getByText('list_folders'));

    // Change to different tool
    await user.click(screen.getByText('search_emails'));

    // Mock cleared data
    executeMock.data = null;
    rerender(<McpTools />);

    expect(screen.queryByText(/Previous result/i)).not.toBeInTheDocument();
  });

  it('validates required parameters', async () => {
    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    // Select tool with required parameters
    await user.click(screen.getByText('atomic_move_message'));

    // Try to execute without filling required fields
    const executeButton = screen.getByText('Execute');
    await user.click(executeButton);

    // Should show validation error or disable button
    expect(screen.getByLabelText('source_folder')).toHaveAttribute('required');
    expect(screen.getByLabelText('target_folder')).toHaveAttribute('required');
    expect(screen.getByLabelText('uid')).toHaveAttribute('required');
  });

  it('handles tool with no parameters', async () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    const mockMutate = vi.fn();
    useExecuteMcpTool.mockReturnValue({
      mutate: mockMutate,
      isLoading: false,
      error: null,
      data: null,
    });

    const user = userEvent.setup();
    render(<McpTools />, { wrapper: createWrapper() });

    // Select tool without parameters
    await user.click(screen.getByText('list_folders'));

    // Should be able to execute immediately
    await user.click(screen.getByText('Execute'));

    expect(mockMutate).toHaveBeenCalledWith({
      tool: 'list_folders',
      parameters: {},
    });
  });

  it('formats JSON results properly', () => {
    const { useExecuteMcpTool } = require('../../api/hooks');
    useExecuteMcpTool.mockReturnValue({
      mutate: vi.fn(),
      isLoading: false,
      error: null,
      data: {
        success: true,
        data: {
          folders: ['INBOX', 'Sent', 'Drafts', 'Trash'],
          count: 4,
        },
      },
    });

    render(<McpTools />, { wrapper: createWrapper() });

    // Check if JSON is properly formatted
    expect(screen.getByText(/INBOX/i)).toBeInTheDocument();
    expect(screen.getByText(/Sent/i)).toBeInTheDocument();
    expect(screen.getByText(/Drafts/i)).toBeInTheDocument();
    expect(screen.getByText(/4/i)).toBeInTheDocument();
  });

  // Tests for new folder management tools
  describe('Folder Management Tools', () => {
    it('displays create_folder tool', () => {
      render(<McpTools />, { wrapper: createWrapper() });

      expect(screen.getByText('create_folder')).toBeInTheDocument();
      expect(screen.getByText('Create a new email folder in the account')).toBeInTheDocument();
    });

    it('displays delete_folder tool', () => {
      render(<McpTools />, { wrapper: createWrapper() });

      expect(screen.getByText('delete_folder')).toBeInTheDocument();
      expect(screen.getByText('Delete an email folder from the account')).toBeInTheDocument();
    });

    it('displays rename_folder tool', () => {
      render(<McpTools />, { wrapper: createWrapper() });

      expect(screen.getByText('rename_folder')).toBeInTheDocument();
      expect(screen.getByText('Rename an email folder in the account')).toBeInTheDocument();
    });

    it('executes create_folder with folder_name parameter', async () => {
      const { useExecuteMcpTool } = require('../../api/hooks');
      const mockMutate = vi.fn();
      useExecuteMcpTool.mockReturnValue({
        mutate: mockMutate,
        isLoading: false,
        error: null,
        data: null,
      });

      const user = userEvent.setup();
      render(<McpTools />, { wrapper: createWrapper() });

      // Select create_folder tool
      await user.click(screen.getByText('create_folder'));

      // Fill in folder_name parameter
      await user.type(screen.getByLabelText('folder_name'), 'INBOX.Projects');

      // Execute
      await user.click(screen.getByText('Execute'));

      expect(mockMutate).toHaveBeenCalledWith({
        tool: 'create_folder',
        parameters: {
          folder_name: 'INBOX.Projects',
        },
      });
    });

    it('executes delete_folder with folder_name parameter', async () => {
      const { useExecuteMcpTool } = require('../../api/hooks');
      const mockMutate = vi.fn();
      useExecuteMcpTool.mockReturnValue({
        mutate: mockMutate,
        isLoading: false,
        error: null,
        data: null,
      });

      const user = userEvent.setup();
      render(<McpTools />, { wrapper: createWrapper() });

      // Select delete_folder tool
      await user.click(screen.getByText('delete_folder'));

      // Fill in folder_name parameter
      await user.type(screen.getByLabelText('folder_name'), 'INBOX.OldFolder');

      // Execute
      await user.click(screen.getByText('Execute'));

      expect(mockMutate).toHaveBeenCalledWith({
        tool: 'delete_folder',
        parameters: {
          folder_name: 'INBOX.OldFolder',
        },
      });
    });

    it('executes rename_folder with old_name and new_name parameters', async () => {
      const { useExecuteMcpTool } = require('../../api/hooks');
      const mockMutate = vi.fn();
      useExecuteMcpTool.mockReturnValue({
        mutate: mockMutate,
        isLoading: false,
        error: null,
        data: null,
      });

      const user = userEvent.setup();
      render(<McpTools />, { wrapper: createWrapper() });

      // Select rename_folder tool
      await user.click(screen.getByText('rename_folder'));

      // Fill in parameters
      await user.type(screen.getByLabelText('old_name'), 'INBOX.Temp');
      await user.type(screen.getByLabelText('new_name'), 'INBOX.Archive');

      // Execute
      await user.click(screen.getByText('Execute'));

      expect(mockMutate).toHaveBeenCalledWith({
        tool: 'rename_folder',
        parameters: {
          old_name: 'INBOX.Temp',
          new_name: 'INBOX.Archive',
        },
      });
    });

    it('shows success result for create_folder', () => {
      const { useExecuteMcpTool } = require('../../api/hooks');
      useExecuteMcpTool.mockReturnValue({
        mutate: vi.fn(),
        isLoading: false,
        error: null,
        data: {
          success: true,
          data: {
            folder_name: 'INBOX.Projects',
            account_id: 'user@example.com',
          },
        },
      });

      render(<McpTools />, { wrapper: createWrapper() });

      expect(screen.getByText(/Result:/i)).toBeInTheDocument();
      expect(screen.getByText(/INBOX.Projects/i)).toBeInTheDocument();
    });

    it('handles folder management errors gracefully', () => {
      const { useExecuteMcpTool } = require('../../api/hooks');
      useExecuteMcpTool.mockReturnValue({
        mutate: vi.fn(),
        isLoading: false,
        error: new Error('Failed to create folder: Folder already exists'),
        data: null,
      });

      render(<McpTools />, { wrapper: createWrapper() });

      expect(screen.getByText(/Folder already exists/i)).toBeInTheDocument();
    });
  });
});