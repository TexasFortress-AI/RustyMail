import { useState } from 'react';
import { ConnectionAttempt, ConnectionStatus } from '../../types';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '../../components/ui/popover';
import {
  CheckCircle2,
  XCircle,
  HelpCircle,
  Copy,
  ChevronDown,
} from 'lucide-react';
import { useToast } from '../../components/ui/use-toast';

interface ConnectionStatusIndicatorProps {
  label: string; // e.g., "IMAP" or "SMTP"
  attempt: ConnectionAttempt;
  compact?: boolean; // Show just the dot without label
}

export function ConnectionStatusIndicator({
  label,
  attempt,
  compact = false,
}: ConnectionStatusIndicatorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const { toast } = useToast();

  const getStatusColor = (status: ConnectionStatus) => {
    switch (status) {
      case 'success':
        return 'text-green-500';
      case 'failed':
        return 'text-red-500';
      case 'unknown':
        return 'text-gray-400';
    }
  };

  const getStatusIcon = (status: ConnectionStatus) => {
    const className = `h-4 w-4 ${getStatusColor(status)}`;
    switch (status) {
      case 'success':
        return <CheckCircle2 className={className} />;
      case 'failed':
        return <XCircle className={className} />;
      case 'unknown':
        return <HelpCircle className={className} />;
    }
  };

  const getStatusText = (status: ConnectionStatus) => {
    switch (status) {
      case 'success':
        return 'Connected';
      case 'failed':
        return 'Failed';
      case 'unknown':
        return 'Unknown';
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      const diffHours = Math.floor(diffMs / 3600000);
      const diffDays = Math.floor(diffMs / 86400000);

      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      if (diffHours < 24) return `${diffHours}h ago`;
      if (diffDays < 7) return `${diffDays}d ago`;

      return date.toLocaleDateString();
    } catch {
      return 'Unknown';
    }
  };

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(attempt.message);
      toast({
        title: 'Copied',
        description: 'Message copied to clipboard',
      });
    } catch (error) {
      toast({
        title: 'Error',
        description: 'Failed to copy to clipboard',
        variant: 'destructive',
      });
    }
  };

  if (compact) {
    return (
      <Popover open={isOpen} onOpenChange={setIsOpen}>
        <PopoverTrigger asChild>
          <button
            className="inline-flex items-center justify-center rounded-full hover:bg-gray-100 p-1 transition-colors"
            aria-label={`${label} status: ${getStatusText(attempt.status)}`}
          >
            {getStatusIcon(attempt.status)}
          </button>
        </PopoverTrigger>
        <PopoverContent className="w-80" align="start">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <h4 className="font-semibold text-sm">{label} Connection</h4>
              <Badge
                variant={
                  attempt.status === 'success'
                    ? 'default'
                    : attempt.status === 'failed'
                    ? 'destructive'
                    : 'secondary'
                }
                className="text-xs"
              >
                {getStatusText(attempt.status)}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground">
              Last attempt: {formatTimestamp(attempt.timestamp)}
            </p>
            <div className="bg-muted p-2 rounded text-xs font-mono break-all">
              {attempt.message}
            </div>
            <Button
              onClick={copyToClipboard}
              variant="outline"
              size="sm"
              className="w-full"
            >
              <Copy className="mr-2 h-3 w-3" />
              Copy Message
            </Button>
          </div>
        </PopoverContent>
      </Popover>
    );
  }

  return (
    <Popover open={isOpen} onOpenChange={setIsOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          className="h-auto py-1 px-2 hover:bg-gray-100"
        >
          <div className="flex items-center gap-1.5">
            {getStatusIcon(attempt.status)}
            <span className="text-xs font-medium">{label}</span>
            <ChevronDown className="h-3 w-3 opacity-50" />
          </div>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-80" align="start">
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="font-semibold text-sm">{label} Connection</h4>
            <Badge
              variant={
                attempt.status === 'success'
                  ? 'default'
                  : attempt.status === 'failed'
                  ? 'destructive'
                  : 'secondary'
              }
              className="text-xs"
            >
              {getStatusText(attempt.status)}
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            Last attempt: {formatTimestamp(attempt.timestamp)}
          </p>
          <div className="bg-muted p-2 rounded text-xs font-mono break-all max-h-32 overflow-y-auto">
            {attempt.message}
          </div>
          <Button
            onClick={copyToClipboard}
            variant="outline"
            size="sm"
            className="w-full"
          >
            <Copy className="mr-2 h-3 w-3" />
            Copy Message
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
