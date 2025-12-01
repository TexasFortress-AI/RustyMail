// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.


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
    retry: false, // Prevent re-rendering loops on block errors
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
    retry: false,
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
      // Also invalidate models since they change with provider
      queryClient.invalidateQueries({ queryKey: ['aiModels'] });
    },
  });
}

// Hook for fetching AI models
export function useAiModels() {
  return useQuery({
    queryKey: ['aiModels'],
    queryFn: async () => {
      return apiClient.getAiModels();
    },
    retry: false,
  });
}

// Hook for setting AI model
export function useSetAiModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (modelName: string) => {
      return apiClient.setAiModel(modelName);
    },
    onSuccess: () => {
      // Invalidate and refetch AI models list
      queryClient.invalidateQueries({ queryKey: ['aiModels'] });
    },
  });
}

// Hook for fetching MCP tools
export function useMcpTools(variant: 'low-level' | 'high-level') {
  return useQuery({
    queryKey: ['mcpTools', variant],
    queryFn: async () => {
      return apiClient.getMcpTools(variant);
    },
    retry: false,
  });
}

// Hook for fetching AI model configurations (tool-calling and drafting)
export function useModelConfigs() {
  return useQuery({
    queryKey: ['modelConfigs'],
    queryFn: async () => {
      return apiClient.getModelConfigs();
    },
    retry: false,
  });
}

// Hook for setting AI model configuration
export function useSetModelConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (config: {
      role: string;
      provider: string;
      model_name: string;
      base_url?: string;
      api_key?: string;
    }) => {
      return apiClient.setModelConfig(config);
    },
    onSuccess: () => {
      // Invalidate and refetch model configs
      queryClient.invalidateQueries({ queryKey: ['modelConfigs'] });
    },
  });
}

// Hook for fetching models for a specific provider
export function useModelsForProvider(provider: string | null) {
  return useQuery({
    queryKey: ['modelsForProvider', provider],
    queryFn: async () => {
      if (!provider) return { provider: '', available_models: [] };
      return apiClient.getModelsForProvider(provider);
    },
    enabled: !!provider,
    retry: false,
  });
}

// Hook for fetching jobs list
export function useJobs(params?: { status?: string; limit?: number }) {
  return useQuery({
    queryKey: ['jobs', params?.status, params?.limit],
    queryFn: async () => {
      return apiClient.getJobs(params);
    },
    refetchInterval: 5000, // Refresh every 5 seconds to show job progress
    retry: false,
  });
}

// Hook for fetching a single job
export function useJob(jobId: string | null) {
  return useQuery({
    queryKey: ['job', jobId],
    queryFn: async () => {
      if (!jobId) return null;
      return apiClient.getJob(jobId);
    },
    enabled: !!jobId,
    refetchInterval: 2000, // Refresh quickly while viewing a specific job
    retry: false,
  });
}

// Hook for cancelling a job
export function useCancelJob() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (jobId: string) => {
      return apiClient.cancelJob(jobId);
    },
    onSuccess: () => {
      // Invalidate jobs list to refresh status
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    },
  });
}

// Hook for starting a process_email_instructions job
export function useStartProcessEmailsJob() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      instruction: string;
      account_id: string;
      folder?: string;
    }) => {
      return apiClient.startProcessEmailsJob(params);
    },
    onSuccess: () => {
      // Invalidate jobs list to show the new job
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    },
  });
}
