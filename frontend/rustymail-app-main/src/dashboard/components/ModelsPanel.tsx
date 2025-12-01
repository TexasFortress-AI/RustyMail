// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useState, useEffect, useMemo, useRef } from 'react';
import { Bot, Settings, Loader2, Save, Search, X } from 'lucide-react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { useModelConfigs, useSetModelConfig, useAiProviders, useModelsForProvider } from '../api/hooks';

interface ModelConfig {
  role: string;
  provider: string;
  model_name: string;
  base_url: string | null;
  api_key: string | null;
  additional_config: string | null;
}

interface ModelConfigEditorProps {
  title: string;
  description: string;
  role: string;
  config: ModelConfig | undefined;
  providers: Array<{ name: string; enabled: boolean }>;
  onSave: (config: {
    role: string;
    provider: string;
    model_name: string;
    base_url?: string;
    api_key?: string;
  }) => void;
  isSaving: boolean;
}

const ModelConfigEditor: React.FC<ModelConfigEditorProps> = ({
  title,
  description,
  role,
  config,
  providers,
  onSave,
  isSaving,
}) => {
  const [selectedProvider, setSelectedProvider] = useState<string>(config?.provider || '');
  const [selectedModel, setSelectedModel] = useState<string>(config?.model_name || '');
  const [baseUrl, setBaseUrl] = useState<string>(config?.base_url || '');
  const [apiKey, setApiKey] = useState<string>(config?.api_key || '');
  const [modelSearchQuery, setModelSearchQuery] = useState('');
  const [isModelSelectOpen, setIsModelSelectOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Fetch models for the selected provider
  const { data: modelsData, isLoading: isLoadingModels } = useModelsForProvider(selectedProvider);

  // Update local state when config changes
  useEffect(() => {
    if (config) {
      setSelectedProvider(config.provider);
      setSelectedModel(config.model_name);
      setBaseUrl(config.base_url || '');
      setApiKey(config.api_key || '');
    }
  }, [config]);

  // Focus search input when dropdown opens
  useEffect(() => {
    if (isModelSelectOpen && searchInputRef.current) {
      setTimeout(() => {
        searchInputRef.current?.focus();
      }, 0);
    }
  }, [isModelSelectOpen]);

  // Filter models based on search query
  const filteredModels = useMemo(() => {
    if (!modelsData?.available_models) return [];
    if (!modelSearchQuery.trim()) return modelsData.available_models;

    return modelsData.available_models.filter(model =>
      model.toLowerCase().includes(modelSearchQuery.toLowerCase())
    );
  }, [modelsData?.available_models, modelSearchQuery]);

  const handleProviderChange = (value: string) => {
    setSelectedProvider(value);
    setSelectedModel(''); // Reset model when provider changes
    setModelSearchQuery('');
  };

  const handleSave = () => {
    if (!selectedProvider || !selectedModel) return;

    onSave({
      role,
      provider: selectedProvider,
      model_name: selectedModel,
      base_url: baseUrl || undefined,
      api_key: apiKey || undefined,
    });
  };

  const enabledProviders = providers.filter(p => p.enabled);
  const hasChanges = config && (
    selectedProvider !== config.provider ||
    selectedModel !== config.model_name ||
    baseUrl !== (config.base_url || '') ||
    apiKey !== (config.api_key || '')
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-5 w-5" />
          {title}
        </CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Provider Selection */}
        <div className="space-y-2">
          <Label htmlFor={`${role}-provider`}>AI Provider</Label>
          <Select value={selectedProvider} onValueChange={handleProviderChange}>
            <SelectTrigger id={`${role}-provider`}>
              <SelectValue placeholder="Select a provider" />
            </SelectTrigger>
            <SelectContent>
              {enabledProviders.map((provider) => (
                <SelectItem key={provider.name} value={provider.name}>
                  {provider.name.charAt(0).toUpperCase() + provider.name.slice(1)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Model Selection with Search */}
        <div className="space-y-2">
          <Label htmlFor={`${role}-model`}>Model</Label>
          {isLoadingModels ? (
            <div className="flex items-center gap-2 h-10 px-3 border rounded-md bg-muted">
              <Loader2 className="h-4 w-4 animate-spin" />
              <span className="text-sm text-muted-foreground">Loading models...</span>
            </div>
          ) : !selectedProvider ? (
            <div className="h-10 px-3 border rounded-md bg-muted flex items-center">
              <span className="text-sm text-muted-foreground">Select a provider first</span>
            </div>
          ) : (
            <Select
              value={selectedModel}
              onValueChange={setSelectedModel}
              onOpenChange={(open) => {
                setIsModelSelectOpen(open);
                if (!open) setModelSearchQuery('');
              }}
            >
              <SelectTrigger id={`${role}-model`}>
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                <div className="p-2 border-b" onPointerDown={(e) => e.stopPropagation()}>
                  <div className="relative">
                    <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground pointer-events-none" />
                    <Input
                      ref={searchInputRef}
                      placeholder="Search models..."
                      value={modelSearchQuery}
                      onChange={(e) => setModelSearchQuery(e.target.value)}
                      onKeyDown={(e) => e.stopPropagation()}
                      onMouseDown={(e) => e.stopPropagation()}
                      className="pl-8 pr-8 h-8"
                      autoFocus={true}
                    />
                    {modelSearchQuery && (
                      <button
                        type="button"
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          setModelSearchQuery('');
                          setTimeout(() => searchInputRef.current?.focus(), 0);
                        }}
                        onMouseDown={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                        }}
                        className="absolute right-2 top-2.5 h-4 w-4 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
                      >
                        <X className="h-4 w-4" />
                      </button>
                    )}
                  </div>
                </div>
                <div className="max-h-60 overflow-y-auto">
                  {filteredModels.length > 0 ? (
                    filteredModels.map((model) => (
                      <SelectItem key={model} value={model}>
                        <div className="truncate max-w-[300px]" title={model}>
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
          )}
        </div>

        {/* Base URL (optional) */}
        <div className="space-y-2">
          <Label htmlFor={`${role}-base-url`}>Base URL (optional)</Label>
          <Input
            id={`${role}-base-url`}
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="e.g., http://localhost:11434"
          />
          <p className="text-xs text-muted-foreground">
            Override the default API endpoint for this provider
          </p>
        </div>

        {/* API Key (optional) */}
        <div className="space-y-2">
          <Label htmlFor={`${role}-api-key`}>API Key (optional)</Label>
          <Input
            id={`${role}-api-key`}
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="Enter API key"
          />
          <p className="text-xs text-muted-foreground">
            API key for commercial providers (leave empty to use environment variable)
          </p>
        </div>

        {/* Save Button */}
        <Button
          onClick={handleSave}
          disabled={!selectedProvider || !selectedModel || isSaving || !hasChanges}
          className="w-full"
        >
          {isSaving ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <Save className="h-4 w-4 mr-2" />
              Save Configuration
            </>
          )}
        </Button>

        {/* Current Config Display */}
        {config && (
          <div className="mt-4 p-3 bg-muted rounded-md">
            <p className="text-xs font-semibold text-muted-foreground mb-1">CURRENT CONFIGURATION</p>
            <p className="text-sm">
              <span className="font-medium">{config.provider}</span>
              {' / '}
              <span className="font-mono text-xs">{config.model_name}</span>
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

const ModelsPanel: React.FC = () => {
  const { data: modelConfigs, isLoading: isLoadingConfigs } = useModelConfigs();
  const { data: aiProviders, isLoading: isLoadingProviders } = useAiProviders();
  const setModelConfigMutation = useSetModelConfig();

  const toolCallingConfig = modelConfigs?.configs.find(c => c.role === 'tool_calling');
  const draftingConfig = modelConfigs?.configs.find(c => c.role === 'drafting');

  const handleSave = (config: {
    role: string;
    provider: string;
    model_name: string;
    base_url?: string;
    api_key?: string;
  }) => {
    setModelConfigMutation.mutate(config);
  };

  if (isLoadingConfigs || isLoadingProviders) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-muted-foreground">Loading model configurations...</p>
        </div>
      </div>
    );
  }

  const providers = aiProviders?.availableProviders || [];

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-4xl mx-auto space-y-6">
        <div className="flex items-center gap-3 mb-6">
          <Settings className="h-6 w-6 text-primary" />
          <div>
            <h2 className="text-2xl font-bold">AI Model Configuration</h2>
            <p className="text-muted-foreground">
              Configure the AI models used for email processing and drafting
            </p>
          </div>
        </div>

        <div className="grid gap-6 md:grid-cols-2">
          {/* Tool Calling Model Configuration */}
          <ModelConfigEditor
            title="Email Processing Model"
            description="Used for understanding instructions and routing email workflows"
            role="tool_calling"
            config={toolCallingConfig}
            providers={providers}
            onSave={handleSave}
            isSaving={setModelConfigMutation.isPending}
          />

          {/* Drafting Model Configuration */}
          <ModelConfigEditor
            title="Drafting Model"
            description="Used for generating email drafts and replies"
            role="drafting"
            config={draftingConfig}
            providers={providers}
            onSave={handleSave}
            isSaving={setModelConfigMutation.isPending}
          />
        </div>
      </div>
    </div>
  );
};

export default ModelsPanel;
