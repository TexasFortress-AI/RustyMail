
import { useState, useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { SSEEvent } from '@/types';
import { initEventSource } from '@/dashboard/api/client';
import { useToast } from '@/hooks/use-toast';

// Hook for handling SSE events
export function useSSEEvents() {
  const [events, setEvents] = useState<SSEEvent[]>([]);
  const queryClient = useQueryClient();
  const { toast } = useToast();
  
  useEffect(() => {
    // Initialize the event source
    const cleanup = initEventSource(
      // Stats updated handler
      (data) => {
        queryClient.setQueryData(['stats'], data);
        setEvents(prev => [
          { type: 'stats_updated', timestamp: new Date(), data },
          ...prev.slice(0, 99) // Keep last 100 events
        ]);
      },
      
      // Client connected handler
      (data) => {
        queryClient.invalidateQueries({ queryKey: ['clients'] });
        setEvents(prev => [
          { type: 'client_connected', timestamp: new Date(), data },
          ...prev.slice(0, 99)
        ]);
        
        // Show toast notification
        toast({
          title: "Client Connected",
          description: `Client ${data.client.id} connected`,
          variant: "default",
        });
      },
      
      // Client disconnected handler
      (data) => {
        queryClient.invalidateQueries({ queryKey: ['clients'] });
        setEvents(prev => [
          { type: 'client_disconnected', timestamp: new Date(), data },
          ...prev.slice(0, 99)
        ]);
        
        // Show toast notification
        toast({
          title: "Client Disconnected",
          description: `Client ${data.client.id} disconnected`,
          variant: "default",
        });
      },
      
      // System alert handler
      (data) => {
        setEvents(prev => [
          { type: 'system_alert', timestamp: new Date(), data },
          ...prev.slice(0, 99)
        ]);
        
        // Show toast notification with variant based on alert type
        const variant = data.type === 'error' 
          ? "destructive" 
          : data.type === 'warning' 
            ? "warning" 
            : "default";
            
        toast({
          title: `System ${data.type.charAt(0).toUpperCase() + data.type.slice(1)}`,
          description: data.message,
          variant: variant as any,
        });
      }
    );
    
    // Cleanup when component unmounts
    return cleanup;
  }, [queryClient, toast]);
  
  return events;
}
