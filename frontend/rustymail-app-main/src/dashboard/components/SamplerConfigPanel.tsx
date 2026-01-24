// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useState, useEffect, useCallback } from 'react';
import { Sliders, Save, Loader2, RotateCcw, Trash2, Info } from 'lucide-react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Slider } from '@/components/ui/slider';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  useSamplerConfig,
  useSamplerConfigs,
  useSetSamplerConfig,
  useDeleteSamplerConfig,
  useSamplerDefaults,
  useAiProviders,
  useModelsForProvider,
} from '../api/hooks';

// Provider-specific field visibility
const PROVIDER_FIELDS: Record<string, string[]> = {
  ollama: ['temperature', 'topP', 'topK', 'repeatPenalty', 'numCtx', 'thinkMode', 'stopSequences'],
  llamacpp: ['temperature', 'topP', 'topK', 'minP', 'repeatPenalty', 'numCtx', 'thinkMode', 'stopSequences'],
  lmstudio: ['temperature', 'topP', 'topK', 'minP', 'repeatPenalty', 'numCtx', 'maxTokens', 'stopSequences'],
  openai: ['temperature', 'topP', 'maxTokens', 'stopSequences'],
  anthropic: ['temperature', 'topP', 'maxTokens', 'stopSequences'],
  default: ['temperature', 'topP', 'topK', 'minP', 'repeatPenalty', 'numCtx', 'maxTokens', 'thinkMode', 'stopSequences'],
};

interface FormState {
  temperature: number;
  topP: number;
  topK: number | undefined;
  minP: number;
  repeatPenalty: number;
  numCtx: number;
  maxTokens: number | undefined;
  thinkMode: boolean;
  stopSequences: string;
  description: string;
}

const DEFAULT_FORM_STATE: FormState = {
  temperature: 0.7,
  topP: 1.0,
  topK: undefined,
  minP: 0.01,
  repeatPenalty: 1.0,
  numCtx: 8192,
  maxTokens: undefined,
  thinkMode: false,
  stopSequences: '',
  description: '',
};

const FieldTooltip: React.FC<{ text: string }> = ({ text }) => (
  <TooltipProvider>
    <Tooltip>
      <TooltipTrigger asChild>
        <Info className="h-4 w-4 text-muted-foreground cursor-help" />
      </TooltipTrigger>
      <TooltipContent className="max-w-xs">
        <p>{text}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
);

const SamplerConfigPanel: React.FC = () => {
  const [selectedProvider, setSelectedProvider] = useState<string>('');
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [formState, setFormState] = useState<FormState>(DEFAULT_FORM_STATE);
  const [hasChanges, setHasChanges] = useState(false);

  // API hooks
  const { data: providersData, isLoading: isLoadingProviders } = useAiProviders();
  const { data: modelsData, isLoading: isLoadingModels } = useModelsForProvider(selectedProvider || null);
  const { data: samplerConfig, isLoading: isLoadingConfig } = useSamplerConfig(
    selectedProvider || null,
    selectedModel || null
  );
  const { data: existingConfigs } = useSamplerConfigs();
  const { data: defaultsData } = useSamplerDefaults();
  const setSamplerConfigMutation = useSetSamplerConfig();
  const deleteSamplerConfigMutation = useDeleteSamplerConfig();

  // Get visible fields for current provider
  const visibleFields = PROVIDER_FIELDS[selectedProvider] || PROVIDER_FIELDS.default;

  // Load config when provider/model changes
  useEffect(() => {
    if (samplerConfig) {
      setFormState({
        temperature: samplerConfig.temperature ?? DEFAULT_FORM_STATE.temperature,
        topP: samplerConfig.top_p ?? DEFAULT_FORM_STATE.topP,
        topK: samplerConfig.top_k ?? undefined,
        minP: samplerConfig.min_p ?? DEFAULT_FORM_STATE.minP,
        repeatPenalty: samplerConfig.repeat_penalty ?? DEFAULT_FORM_STATE.repeatPenalty,
        numCtx: samplerConfig.num_ctx ?? DEFAULT_FORM_STATE.numCtx,
        maxTokens: samplerConfig.max_tokens ?? undefined,
        thinkMode: samplerConfig.think_mode ?? false,
        stopSequences: (samplerConfig.stop_sequences || []).join('\n'),
        description: samplerConfig.description ?? '',
      });
      setHasChanges(false);
    } else if (defaultsData && selectedProvider && selectedModel) {
      // Load environment defaults for new configs
      setFormState({
        temperature: defaultsData.temperature ?? DEFAULT_FORM_STATE.temperature,
        topP: defaultsData.top_p ?? DEFAULT_FORM_STATE.topP,
        topK: defaultsData.top_k ?? undefined,
        minP: defaultsData.min_p ?? DEFAULT_FORM_STATE.minP,
        repeatPenalty: defaultsData.repeat_penalty ?? DEFAULT_FORM_STATE.repeatPenalty,
        numCtx: defaultsData.num_ctx ?? DEFAULT_FORM_STATE.numCtx,
        maxTokens: defaultsData.max_tokens ?? undefined,
        thinkMode: defaultsData.think_mode ?? false,
        stopSequences: '',
        description: '',
      });
      setHasChanges(false);
    }
  }, [samplerConfig, defaultsData, selectedProvider, selectedModel]);

  const updateFormField = useCallback(<K extends keyof FormState>(field: K, value: FormState[K]) => {
    setFormState(prev => ({ ...prev, [field]: value }));
    setHasChanges(true);
  }, []);

  const handleProviderChange = (value: string) => {
    setSelectedProvider(value);
    setSelectedModel('');
    setHasChanges(false);
  };

  const handleModelChange = (value: string) => {
    setSelectedModel(value);
    setHasChanges(false);
  };

  const handleSave = async () => {
    if (!selectedProvider || !selectedModel) return;

    await setSamplerConfigMutation.mutateAsync({
      provider: selectedProvider,
      model_name: selectedModel,
      temperature: formState.temperature,
      top_p: formState.topP,
      top_k: formState.topK,
      min_p: formState.minP,
      repeat_penalty: formState.repeatPenalty,
      num_ctx: formState.numCtx,
      max_tokens: formState.maxTokens,
      think_mode: formState.thinkMode,
      stop_sequences: formState.stopSequences.split('\n').filter(s => s.trim()),
      description: formState.description || undefined,
    });

    setHasChanges(false);
  };

  const handleReset = () => {
    if (defaultsData) {
      setFormState({
        temperature: defaultsData.temperature ?? DEFAULT_FORM_STATE.temperature,
        topP: defaultsData.top_p ?? DEFAULT_FORM_STATE.topP,
        topK: defaultsData.top_k ?? undefined,
        minP: defaultsData.min_p ?? DEFAULT_FORM_STATE.minP,
        repeatPenalty: defaultsData.repeat_penalty ?? DEFAULT_FORM_STATE.repeatPenalty,
        numCtx: defaultsData.num_ctx ?? DEFAULT_FORM_STATE.numCtx,
        maxTokens: defaultsData.max_tokens ?? undefined,
        thinkMode: defaultsData.think_mode ?? false,
        stopSequences: '',
        description: '',
      });
      setHasChanges(true);
    }
  };

  const handleDelete = async () => {
    if (!selectedProvider || !selectedModel) return;

    if (window.confirm(`Delete sampler config for ${selectedProvider}/${selectedModel}?`)) {
      await deleteSamplerConfigMutation.mutateAsync({
        provider: selectedProvider,
        modelName: selectedModel,
      });
      setSelectedModel('');
      setHasChanges(false);
    }
  };

  const enabledProviders = providersData?.availableProviders?.filter(p => p.enabled) || [];

  // Check if current model has a saved config
  const hasSavedConfig = existingConfigs?.configs?.some(
    c => c.provider === selectedProvider && c.model_name === selectedModel
  );

  if (isLoadingProviders) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-muted-foreground">Loading providers...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-4xl mx-auto space-y-6">
        <div className="flex items-center gap-3 mb-6">
          <Sliders className="h-6 w-6 text-primary" />
          <div>
            <h2 className="text-2xl font-bold">Sampler Configuration</h2>
            <p className="text-muted-foreground">
              Configure AI model sampler settings for local LLM inference
            </p>
          </div>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Select Model</CardTitle>
            <CardDescription>
              Choose a provider and model to configure sampler settings
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 md:grid-cols-2">
              {/* Provider Selection */}
              <div className="space-y-2">
                <Label htmlFor="provider">AI Provider</Label>
                <Select value={selectedProvider} onValueChange={handleProviderChange}>
                  <SelectTrigger id="provider">
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

              {/* Model Selection */}
              <div className="space-y-2">
                <Label htmlFor="model">Model</Label>
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
                  <Select value={selectedModel} onValueChange={handleModelChange}>
                    <SelectTrigger id="model">
                      <SelectValue placeholder="Select a model" />
                    </SelectTrigger>
                    <SelectContent>
                      {modelsData?.available_models?.map((model) => (
                        <SelectItem key={model} value={model}>
                          <div className="truncate max-w-[250px]" title={model}>
                            {model}
                          </div>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </div>
            </div>
          </CardContent>
        </Card>

        {selectedProvider && selectedModel && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                Sampler Settings
                {isLoadingConfig && <Loader2 className="h-4 w-4 animate-spin" />}
                {hasSavedConfig && (
                  <span className="text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 px-2 py-0.5 rounded">
                    Saved
                  </span>
                )}
              </CardTitle>
              <CardDescription>
                Configure sampling parameters for {selectedProvider}/{selectedModel}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {/* Temperature */}
              {visibleFields.includes('temperature') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Temperature: {formState.temperature.toFixed(2)}</Label>
                    <FieldTooltip text="Controls randomness. 0 = deterministic, 2 = very random. Lower values make output more focused." />
                  </div>
                  <Slider
                    value={[formState.temperature]}
                    onValueChange={([v]) => updateFormField('temperature', v)}
                    min={0}
                    max={2}
                    step={0.05}
                  />
                </div>
              )}

              {/* Top P */}
              {visibleFields.includes('topP') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Top P: {formState.topP.toFixed(2)}</Label>
                    <FieldTooltip text="Nucleus sampling. Only consider tokens with cumulative probability up to this value. 1.0 = disabled." />
                  </div>
                  <Slider
                    value={[formState.topP]}
                    onValueChange={([v]) => updateFormField('topP', v)}
                    min={0}
                    max={1}
                    step={0.01}
                  />
                </div>
              )}

              {/* Top K */}
              {visibleFields.includes('topK') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Top K (optional)</Label>
                    <FieldTooltip text="Only consider the top K most likely tokens. Leave empty to disable." />
                  </div>
                  <Input
                    type="number"
                    placeholder="Empty = disabled"
                    value={formState.topK ?? ''}
                    onChange={(e) => updateFormField('topK', e.target.value ? parseInt(e.target.value) : undefined)}
                    min={1}
                    max={100}
                  />
                </div>
              )}

              {/* Min P */}
              {visibleFields.includes('minP') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Min P: {formState.minP.toFixed(3)}</Label>
                    <FieldTooltip text="Minimum probability threshold. Tokens below this probability are filtered out. Recommended: 0.01" />
                  </div>
                  <Slider
                    value={[formState.minP]}
                    onValueChange={([v]) => updateFormField('minP', v)}
                    min={0}
                    max={0.5}
                    step={0.005}
                  />
                </div>
              )}

              {/* Repeat Penalty */}
              {visibleFields.includes('repeatPenalty') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Repeat Penalty: {formState.repeatPenalty.toFixed(2)}</Label>
                    <FieldTooltip text="Penalizes repetition. 1.0 = disabled. Higher values reduce repetition." />
                  </div>
                  <Slider
                    value={[formState.repeatPenalty]}
                    onValueChange={([v]) => updateFormField('repeatPenalty', v)}
                    min={0}
                    max={2}
                    step={0.05}
                  />
                </div>
              )}

              {/* Context Window */}
              {visibleFields.includes('numCtx') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Context Window (num_ctx)</Label>
                    <FieldTooltip text="Maximum context size in tokens. Larger = more memory but better understanding. Default: 8192" />
                  </div>
                  <Input
                    type="number"
                    value={formState.numCtx}
                    onChange={(e) => updateFormField('numCtx', parseInt(e.target.value) || 2048)}
                    min={512}
                    max={200000}
                    step={1024}
                  />
                  <p className="text-xs text-muted-foreground">
                    Common values: 8192, 32768, 51200, 131072, 200000
                  </p>
                </div>
              )}

              {/* Max Tokens */}
              {visibleFields.includes('maxTokens') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Max Tokens (optional)</Label>
                    <FieldTooltip text="Maximum tokens to generate. Leave empty for no limit." />
                  </div>
                  <Input
                    type="number"
                    placeholder="Empty = no limit"
                    value={formState.maxTokens ?? ''}
                    onChange={(e) => updateFormField('maxTokens', e.target.value ? parseInt(e.target.value) : undefined)}
                    min={1}
                    max={100000}
                  />
                </div>
              )}

              {/* Think Mode */}
              {visibleFields.includes('thinkMode') && (
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Label htmlFor="think-mode">Think Mode</Label>
                    <FieldTooltip text="Enable verbose reasoning output. Disable for cleaner tool-calling responses (recommended for GLM-4, Qwen)." />
                  </div>
                  <Switch
                    id="think-mode"
                    checked={formState.thinkMode}
                    onCheckedChange={(checked) => updateFormField('thinkMode', checked)}
                  />
                </div>
              )}

              {/* Stop Sequences */}
              {visibleFields.includes('stopSequences') && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>Stop Sequences (one per line)</Label>
                    <FieldTooltip text="Text sequences that stop generation when encountered." />
                  </div>
                  <Textarea
                    placeholder="Enter stop sequences, one per line"
                    value={formState.stopSequences}
                    onChange={(e) => updateFormField('stopSequences', e.target.value)}
                    rows={3}
                  />
                </div>
              )}

              {/* Description */}
              <div className="space-y-2">
                <Label>Description (optional)</Label>
                <Input
                  placeholder="Notes about this configuration"
                  value={formState.description}
                  onChange={(e) => updateFormField('description', e.target.value)}
                />
              </div>

              {/* Action Buttons */}
              <div className="flex gap-3 pt-4">
                <Button
                  onClick={handleSave}
                  disabled={!hasChanges || setSamplerConfigMutation.isPending}
                  className="flex-1"
                >
                  {setSamplerConfigMutation.isPending ? (
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
                <Button variant="outline" onClick={handleReset}>
                  <RotateCcw className="h-4 w-4 mr-2" />
                  Reset to Defaults
                </Button>
                {hasSavedConfig && (
                  <Button
                    variant="destructive"
                    onClick={handleDelete}
                    disabled={deleteSamplerConfigMutation.isPending}
                  >
                    {deleteSamplerConfigMutation.isPending ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Trash2 className="h-4 w-4" />
                    )}
                  </Button>
                )}
              </div>

              {hasChanges && (
                <p className="text-xs text-amber-600 dark:text-amber-400">
                  You have unsaved changes
                </p>
              )}
            </CardContent>
          </Card>
        )}

        {/* List of existing configs */}
        {existingConfigs?.configs && existingConfigs.configs.length > 0 && (
          <Card>
            <CardHeader>
              <CardTitle>Saved Configurations</CardTitle>
              <CardDescription>
                Click to load and edit a saved configuration
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                {existingConfigs.configs.map((config) => (
                  <button
                    key={`${config.provider}-${config.model_name}`}
                    onClick={() => {
                      setSelectedProvider(config.provider);
                      setSelectedModel(config.model_name);
                    }}
                    className={`w-full p-3 text-left border rounded-md hover:bg-muted transition-colors ${
                      selectedProvider === config.provider && selectedModel === config.model_name
                        ? 'border-primary bg-muted'
                        : ''
                    }`}
                  >
                    <div className="font-medium">{config.provider}</div>
                    <div className="text-sm text-muted-foreground truncate">
                      {config.model_name}
                    </div>
                    {config.description && (
                      <div className="text-xs text-muted-foreground mt-1">
                        {config.description}
                      </div>
                    )}
                  </button>
                ))}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
};

export default SamplerConfigPanel;
