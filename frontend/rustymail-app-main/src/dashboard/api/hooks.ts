
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiClient } from './client';
import { ChatbotQuery, ServerConfig } from '@/types';

// Hook for fetching dashboard stats
export function useStats() {
  return useQuery({
    queryKey: ['stats'],
    queryFn: async () => {
      return apiClient.getStats();
    },
    refetchInterval: 30000, // Refetch every 30 seconds
  });
}

// Hook for fetching client list with pagination and filtering
export function useClients(page: number, limit: number, filter?: string) {
  const queryClient = useQueryClient();
  
  return useQuery({
    queryKey: ['clients', page, limit, filter],
    queryFn: async () => {
      return apiClient.getClients(page, limit, filter);
    },
    placeholderData: (previousData) => previousData, // This replaces keepPreviousData from v4
  });
}

// Hook for fetching server configuration
export function useConfig() {
  return useQuery({
    queryKey: ['config'],
    queryFn: async () => {
      return apiClient.getConfig();
    },
  });
}

// Hook for setting active IMAP adapter
export function useSetActiveAdapter() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (adapterId: string) => {
      return apiClient.setActiveAdapter(adapterId);
    },
    onSuccess: (data: ServerConfig) => {
      queryClient.setQueryData(['config'], data);
      // Also update localStorage for persistence
      localStorage.setItem('activeAdapter', data.activeAdapter.id);
    },
  });
}

// Hook for chatbot queries
export function useChatbotMutation() {
  return useMutation({
    mutationFn: async (query: ChatbotQuery) => {
      return apiClient.queryChatbot(query);
    },
  });
}

// Hook for fetching AI providers
export function useAiProviders() {
  return useQuery({
    queryKey: ['aiProviders'],
    queryFn: async () => {
      return apiClient.getAiProviders();
    },
  });
}

// Hook for setting AI provider
export function useSetAiProvider() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (providerName: string) => {
      return apiClient.setAiProvider(providerName);
    },
    onSuccess: () => {
      // Invalidate and refetch AI providers list
      queryClient.invalidateQueries({ queryKey: ['aiProviders'] });
    },
  });
}
