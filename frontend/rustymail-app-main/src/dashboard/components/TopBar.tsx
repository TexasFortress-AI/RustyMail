
import React from 'react';
import { 
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import { useConfig, useSetActiveAdapter } from '@/dashboard/api/hooks';
import { Loader2 } from 'lucide-react';

const TopBar: React.FC = () => {
  const { data: config, isLoading: isConfigLoading, error: configError } = useConfig();
  const setActiveAdapterMutation = useSetActiveAdapter();

  // Debug logging
  console.log('TopBar config data:', config);
  console.log('TopBar config loading:', isConfigLoading);
  console.log('TopBar config error:', configError);

  // Handle adapter selection
  const handleAdapterChange = (value: string) => {
    setActiveAdapterMutation.mutate(value);
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
          
          {config && (
            <div className="hidden md:flex text-xs text-muted-foreground">
              <span className="px-2 py-1 rounded-md bg-primary/10">
                {config.activeAdapter.name}
              </span>
            </div>
          )}
        </div>
      </div>
    </header>
  );
};

export default TopBar;
