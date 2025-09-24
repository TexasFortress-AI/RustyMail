
import React, { useState, useMemo, useEffect } from 'react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { useConfig, useSetActiveAdapter, useAiProviders, useSetAiProvider, useAiModels, useSetAiModel } from '@/dashboard/api/hooks';
import { Loader2, Bot, Search } from 'lucide-react';

const TopBar: React.FC = () => {
  const { data: config, isLoading: isConfigLoading, error: configError } = useConfig();
  const setActiveAdapterMutation = useSetActiveAdapter();

  // AI Provider hooks
  const { data: aiProviders, isLoading: isAiProvidersLoading } = useAiProviders();
  const setAiProviderMutation = useSetAiProvider();

  // AI Model hooks
  const { data: aiModels, isLoading: isAiModelsLoading } = useAiModels();
  const setAiModelMutation = useSetAiModel();

  // Local state for model search
  const [modelSearchQuery, setModelSearchQuery] = useState('');

  // Filter models based on search query
  const filteredModels = useMemo(() => {
    if (!aiModels?.availableModels) return [];
    if (!modelSearchQuery.trim()) return aiModels.availableModels;

    return aiModels.availableModels.filter(model =>
      model.toLowerCase().includes(modelSearchQuery.toLowerCase())
    );
  }, [aiModels?.availableModels, modelSearchQuery]);

  // Restore saved model when provider changes
  useEffect(() => {
    if (aiProviders?.currentProvider && aiModels?.availableModels && aiModels.availableModels.length > 0) {
      const savedModel = localStorage.getItem(`selectedModel_${aiProviders.currentProvider}`);
      if (savedModel && aiModels.availableModels.includes(savedModel)) {
        // Only set if it's different from current to avoid unnecessary API calls
        if (aiModels.currentModel !== savedModel) {
          setAiModelMutation.mutate(savedModel);
        }
      }
    }
  }, [aiProviders?.currentProvider, aiModels?.availableModels]);

  // Debug logging
  console.log('TopBar config data:', config);
  console.log('TopBar config loading:', isConfigLoading);
  console.log('TopBar config error:', configError);
  console.log('TopBar AI providers data:', aiProviders);
  console.log('TopBar AI models data:', aiModels);

  // Handle adapter selection
  const handleAdapterChange = (value: string) => {
    setActiveAdapterMutation.mutate(value);
  };

  // Handle AI provider selection
  const handleAiProviderChange = (value: string) => {
    // Store the current model selection before switching providers
    if (aiProviders?.currentProvider && aiModels?.currentModel) {
      localStorage.setItem(`selectedModel_${aiProviders.currentProvider}`, aiModels.currentModel);
    }

    setAiProviderMutation.mutate(value);

    // Clear model search when switching providers
    setModelSearchQuery('');
  };

  // Handle AI model selection
  const handleAiModelChange = (value: string) => {
    setAiModelMutation.mutate(value);

    // Store the model selection for this provider
    if (aiProviders?.currentProvider) {
      localStorage.setItem(`selectedModel_${aiProviders.currentProvider}`, value);
    }
  };

  return (
    <header 
      className="w-full bg-white/70 dark:bg-black/30 backdrop-blur-lg border-b border-border sticky top-0 z-10 shadow-sm"
      data-testid="top-bar"
    >
      <div className="container mx-auto px-4 py-3 flex items-center justify-between">
        <div className="flex items-center">
          <div className="font-semibold text-xl text-foreground">
            RustyMail SSE Dashboard
          </div>
        </div>
        
        <div className="flex items-center space-x-4">
          <div className="flex items-center space-x-3">
            <span className="text-sm text-muted-foreground">IMAP Adapter:</span>

            {isConfigLoading ? (
              <div className="flex items-center space-x-2">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                <span className="text-sm text-muted-foreground">Loading...</span>
              </div>
            ) : (
              <Select
                defaultValue={config?.activeAdapter.id}
                onValueChange={handleAdapterChange}
                disabled={setActiveAdapterMutation.isPending}
                data-testid="imap-adapter-selector"
              >
                <SelectTrigger className="w-[180px] h-8">
                  <SelectValue placeholder="Select adapter" />
                </SelectTrigger>
                <SelectContent>
                  {config?.availableAdapters.map((adapter) => (
                    <SelectItem
                      key={adapter.id}
                      value={adapter.id}
                      className="cursor-pointer"
                    >
                      {adapter.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div className="flex items-center space-x-3">
            <Bot className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">AI Provider:</span>

            {isAiProvidersLoading ? (
              <div className="flex items-center space-x-2">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                <span className="text-sm text-muted-foreground">Loading...</span>
              </div>
            ) : (
              <Select
                value={aiProviders?.currentProvider || 'none'}
                onValueChange={handleAiProviderChange}
                disabled={setAiProviderMutation.isPending}
                data-testid="ai-provider-selector"
              >
                <SelectTrigger className="w-[140px] h-8">
                  <SelectValue placeholder="Select AI" />
                </SelectTrigger>
                <SelectContent>
                  {aiProviders?.availableProviders.filter(provider => provider.enabled).map((provider) => (
                    <SelectItem
                      key={provider.name}
                      value={provider.name}
                      className="cursor-pointer"
                    >
                      {provider.name.charAt(0).toUpperCase() + provider.name.slice(1)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div className="flex items-center space-x-3">
            <span className="text-sm text-muted-foreground">Model:</span>

            {isAiModelsLoading ? (
              <div className="flex items-center space-x-2">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                <span className="text-sm text-muted-foreground">Loading...</span>
              </div>
            ) : aiModels?.currentModel && aiModels.availableModels.length > 0 ? (
              <div className="relative">
                <Select
                  value={aiModels.currentModel}
                  onValueChange={handleAiModelChange}
                  disabled={setAiModelMutation.isPending || !aiProviders?.currentProvider}
                  data-testid="ai-model-selector"
                >
                  <SelectTrigger className="w-[200px] h-8">
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    <div className="p-2 border-b">
                      <div className="relative">
                        <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
                        <Input
                          placeholder="Search models..."
                          value={modelSearchQuery}
                          onChange={(e) => setModelSearchQuery(e.target.value)}
                          className="pl-8 h-8"
                          autoFocus={false}
                        />
                      </div>
                    </div>
                    <div className="max-h-60 overflow-y-auto">
                      {filteredModels.length > 0 ? (
                        filteredModels.map((model) => (
                          <SelectItem
                            key={model}
                            value={model}
                            className="cursor-pointer"
                          >
                            <div className="truncate max-w-[180px]" title={model}>
                              {model}
                            </div>
                          </SelectItem>
                        ))
                      ) : (
                        <div className="p-2 text-sm text-muted-foreground text-center">
                          No models found
                        </div>
                      )}
                    </div>
                  </SelectContent>
                </Select>
              </div>
            ) : !aiProviders?.currentProvider ? (
              <div className="px-2 py-1 h-8 flex items-center rounded-md bg-muted text-sm text-muted-foreground">
                No provider
              </div>
            ) : (
              <div className="px-2 py-1 h-8 flex items-center rounded-md bg-muted text-sm text-muted-foreground">
                No models available
              </div>
            )}
          </div>

          {config && (
            <div className="hidden md:flex text-xs text-muted-foreground">
              <span className="px-2 py-1 rounded-md bg-primary/10">
                {config.activeAdapter.name}
              </span>
            </div>
          )}

          {aiProviders?.currentProvider && (
            <div className="hidden md:flex text-xs text-muted-foreground">
              <span className="px-2 py-1 rounded-md bg-secondary/20">
                {aiProviders.currentProvider.charAt(0).toUpperCase() + aiProviders.currentProvider.slice(1)} AI
              </span>
            </div>
          )}
        </div>
      </div>
    </header>
  );
};

export default TopBar;
