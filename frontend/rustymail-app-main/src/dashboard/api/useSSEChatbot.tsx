// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { useState, useCallback, useRef } from 'react';
import { ChatbotResponse } from './types';
import { toast } from '@/hooks/use-toast';

interface StreamChatbotOptions {
  onStart?: (conversationId: string) => void;
  onContent?: (text: string, partial: boolean) => void;
  onComplete?: (response: ChatbotResponse) => void;
  onError?: (error: string) => void;
}

export function useSSEChatbot(options: StreamChatbotOptions = {}) {
  const [isStreaming, setIsStreaming] = useState(false);
  const eventSourceRef = useRef<EventSource | null>(null);
  const accumulatedTextRef = useRef<string>('');

  const streamQuery = useCallback(async (query: string, conversationId?: string) => {
    if (isStreaming) {
      console.warn('Already streaming a response');
      return;
    }

    setIsStreaming(true);
    accumulatedTextRef.current = '';

    try {
      // Send the query via POST with SSE response
      const response = await fetch('/api/dashboard/chatbot/stream', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          query,
          conversation_id: conversationId
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const reader = response.body?.getReader();
      const decoder = new TextDecoder();

      if (!reader) {
        throw new Error('No response body');
      }

      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6);
            if (data === '[DONE]') {
              continue;
            }

            try {
              const parsed = JSON.parse(data);

              switch (parsed.type) {
                case 'start':
                  options.onStart?.(parsed.conversation_id);
                  break;

                case 'content':
                  accumulatedTextRef.current = parsed.text;
                  options.onContent?.(parsed.text, false);

                  // When we get the full content, call onComplete
                  if (parsed.email_data || parsed.followup_suggestions) {
                    options.onComplete?.({
                      text: parsed.text,
                      conversation_id: parsed.conversation_id,
                      emailData: parsed.email_data,
                      followupSuggestions: parsed.followup_suggestions
                    });
                  }
                  break;

                case 'complete':
                  // Streaming complete
                  break;

                case 'error':
                  options.onError?.(parsed.error);
                  toast({
                    title: "Chatbot Error",
                    description: parsed.error,
                    variant: "destructive",
                  });
                  break;
              }
            } catch (e) {
              console.error('Error parsing SSE data:', e);
            }
          }
        }
      }
    } catch (error) {
      console.error('Stream error:', error);
      options.onError?.(error instanceof Error ? error.message : 'Stream failed');
      toast({
        title: "Stream Error",
        description: error instanceof Error ? error.message : "Failed to stream response",
        variant: "destructive",
      });
    } finally {
      setIsStreaming(false);
    }
  }, [isStreaming, options]);

  const stopStreaming = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    setIsStreaming(false);
  }, []);

  return {
    streamQuery,
    stopStreaming,
    isStreaming
  };
}