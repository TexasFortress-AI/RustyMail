
import React from 'react';
import { 
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import { useConfig, useSetActiveAdapter, useAiProviders, useSetAiProvider } from '@/dashboard/api/hooks';
import { Loader2, Bot } from 'lucide-react';

const TopBar: React.FC = () => {
  const { data: config, isLoading: isConfigLoading, error: configError } = useConfig();
  const setActiveAdapterMutation = useSetActiveAdapter();

  // AI Provider hooks
  const { data: aiProviders, isLoading: isAiProvidersLoading } = useAiProviders();
  const setAiProviderMutation = useSetAiProvider();

  // Debug logging
  console.log('TopBar config data:', config);
  console.log('TopBar config loading:', isConfigLoading);
  console.log('TopBar config error:', configError);
  console.log('TopBar AI providers data:', aiProviders);

  // Handle adapter selection
  const handleAdapterChange = (value: string) => {
    setActiveAdapterMutation.mutate(value);
  };

  // Handle AI provider selection
  const handleAiProviderChange = (value: string) => {
    setAiProviderMutation.mutate(value);
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

            {isAiProvidersLoading ? (
              <div className="flex items-center space-x-2">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                <span className="text-sm text-muted-foreground">Loading...</span>
              </div>
            ) : aiProviders?.currentProvider ? (
              <div className="px-2 py-1 h-8 flex items-center rounded-md bg-accent/50 text-sm">
                {aiProviders.availableProviders
                  .find(p => p.name === aiProviders.currentProvider)?.model || 'Unknown'}
              </div>
            ) : (
              <div className="px-2 py-1 h-8 flex items-center rounded-md bg-muted text-sm text-muted-foreground">
                No provider
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
