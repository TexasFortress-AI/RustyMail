
import React, { useState, useRef, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { useChatbotMutation } from '@/dashboard/api/hooks';
import { useSSEChatbot } from '@/dashboard/api/useSSEChatbot';
import { ChatMessage, ChatbotQuery, ChatbotResponse } from '@/types';
import { Send, Bot, User, Loader2, Mail, Folder, Download, Copy, Trash2, Bug, CheckCircle, AlertCircle, XCircle, ChevronDown, ChevronUp } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { toast } from '@/hooks/use-toast';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuCheckboxItem,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';
import { MoreHorizontal } from 'lucide-react';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

// Load conversation from localStorage
const loadConversation = (): ChatMessage[] => {
  try {
    const saved = localStorage.getItem('chatConversation');
    return saved ? JSON.parse(saved) : [];
  } catch (e) {
    console.error('Failed to load conversation:', e);
    return [];
  }
};

// Save conversation to localStorage
const saveConversation = (messages: ChatMessage[]): void => {
  try {
    localStorage.setItem('chatConversation', JSON.stringify(messages));
  } catch (e) {
    console.error('Failed to save conversation:', e);
  }
};

interface DebugInfo {
  request: ChatbotQuery;
  response: ChatbotResponse | null;
  timestamp: string;
  error?: string;
}

interface ChatbotPanelProps {
  currentFolder?: string;
  accountId?: string;
}

const ChatbotPanel: React.FC<ChatbotPanelProps> = ({ currentFolder, accountId }) => {
  const [inputText, setInputText] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>(loadConversation);
  const [conversationId, setConversationId] = useState<string | undefined>(undefined);
  const [useStreaming, setUseStreaming] = useState(false); // Temporarily disable SSE streaming - use HTTP POST
  const [streamingMessage, setStreamingMessage] = useState<ChatMessage | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Debug mode state
  const [debugMode, setDebugMode] = useState(() => {
    return localStorage.getItem('chatbot-debug-mode') === 'true';
  });
  const [debugInfo, setDebugInfo] = useState<DebugInfo | null>(null);
  const [debugPanelOpen, setDebugPanelOpen] = useState(false);

  const chatbotMutation = useChatbotMutation();

  const { streamQuery, isStreaming } = useSSEChatbot({
    onStart: (convId) => {
      setConversationId(convId);
    },
    onContent: (text, partial) => {
      // Update the streaming message with new content
      setStreamingMessage(prev => prev ? {
        ...prev,
        text
      } : null);
    },
    onComplete: (response) => {
      // Finalize the message with complete data
      if (streamingMessage) {
        const finalMessage: ChatMessage = {
          ...streamingMessage,
          text: response.text,
          emailData: response.emailData,
          followupSuggestions: response.followupSuggestions
        };
        setMessages(prev => {
          // Replace the streaming message with the final one
          const newMessages = [...prev];
          newMessages[newMessages.length - 1] = finalMessage;
          return newMessages;
        });
        setStreamingMessage(null);
      }
    },
    onError: (error) => {
      setStreamingMessage(null);
      // Add error message to conversation
      const errorMessage: ChatMessage = {
        id: uuidv4(),
        type: 'ai',
        text: 'Sorry, I encountered an error processing your request. Please try again.',
        timestamp: new Date().toISOString()
      };
      setMessages(prev => [...prev, errorMessage]);
    }
  });
  
  // Save conversation whenever messages change
  useEffect(() => {
    saveConversation(messages);
  }, [messages]);
  
  // Scroll to bottom whenever messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);
  
  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Persist debug mode to localStorage
  useEffect(() => {
    localStorage.setItem('chatbot-debug-mode', String(debugMode));
  }, [debugMode]);

  // Clear conversation when account changes
  const prevAccountIdRef = useRef<string | undefined>(accountId);
  useEffect(() => {
    // Only clear if account actually changed (not on initial mount)
    if (prevAccountIdRef.current && prevAccountIdRef.current !== accountId && messages.length > 0) {
      setMessages([]);
      setConversationId(undefined);
      localStorage.removeItem('chatConversation');
    }
    prevAccountIdRef.current = accountId;
  }, [accountId, messages.length]); // React when accountId changes

  
  // Handle form submission
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!inputText.trim()) return;

    // Add user message to the conversation
    const userMessage: ChatMessage = {
      id: uuidv4(),
      type: 'user',
      text: inputText,
      timestamp: new Date().toISOString()
    };

    setMessages(prev => [...prev, userMessage]);
    const queryText = inputText;
    setInputText('');

    if (useStreaming) {
      // Use SSE streaming
      const aiMessage: ChatMessage = {
        id: uuidv4(),
        type: 'ai',
        text: '',
        timestamp: new Date().toISOString()
      };
      setStreamingMessage(aiMessage);
      setMessages(prev => [...prev, aiMessage]);

      // Stream the query
      await streamQuery(queryText, conversationId);
    } else {
      // Use traditional HTTP POST
      const request: ChatbotQuery = {
        query: queryText,
        conversation_id: conversationId,
        current_folder: currentFolder,
        account_id: accountId
      };

      // Capture debug info if debug mode is enabled
      if (debugMode) {
        setDebugInfo({
          request,
          response: null,
          timestamp: new Date().toISOString()
        });
      }

      chatbotMutation.mutate(
        request,
        {
          onSuccess: (response) => {
            // Update debug info with response
            if (debugMode) {
              setDebugInfo(prev => prev ? { ...prev, response } : null);
            }

            // Add AI response to the conversation
            const aiMessage: ChatMessage = {
              id: uuidv4(),
              type: 'ai',
              text: response.text,
              timestamp: new Date().toISOString(),
              emailData: response.emailData,
              followupSuggestions: response.followupSuggestions
            };

            setMessages(prev => [...prev, aiMessage]);
            setConversationId(response.conversation_id);
          },
          onError: (error) => {
            // Update debug info with error
            if (debugMode) {
              setDebugInfo(prev => prev ? {
                ...prev,
                error: error instanceof Error ? error.message : "Failed to get response"
              } : null);
            }

            // Show error toast
            toast({
              title: "Chatbot Error",
              description: error instanceof Error ? error.message : "Failed to get response",
              variant: "destructive",
            });

            // Add error message to conversation
            const errorMessage: ChatMessage = {
              id: uuidv4(),
              type: 'ai',
              text: 'Sorry, I encountered an error processing your request. Please try again.',
              timestamp: new Date().toISOString()
            };

            setMessages(prev => [...prev, errorMessage]);
          }
        }
      );
    }
  };
  
  // Handle quick reply click
  const handleQuickReplyClick = (text: string) => {
    setInputText(text);
    inputRef.current?.focus();
  };
  
  // Clear conversation
  const handleClearConversation = () => {
    setMessages([]);
    setConversationId(undefined);
    localStorage.removeItem('chatConversation');
    toast({
      description: "Conversation cleared",
    });
  };

  // Export conversation as text
  const handleExportText = () => {
    const text = messages.map(msg => {
      const sender = msg.type === 'user' ? 'You' : 'AI Assistant';
      const timestamp = new Date(msg.timestamp).toLocaleString();
      return `[${timestamp}] ${sender}: ${msg.text}`;
    }).join('\n\n');

    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `chat-export-${new Date().toISOString().split('T')[0]}.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast({
      description: "Conversation exported as text file",
    });
  };

  // Export conversation as JSON
  const handleExportJSON = () => {
    const json = JSON.stringify(messages, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `chat-export-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast({
      description: "Conversation exported as JSON file",
    });
  };

  // Copy conversation to clipboard
  const handleCopyToClipboard = async () => {
    const text = messages.map(msg => {
      const sender = msg.type === 'user' ? 'You' : 'AI Assistant';
      return `${sender}: ${msg.text}`;
    }).join('\n\n');

    try {
      await navigator.clipboard.writeText(text);
      toast({
        description: "Conversation copied to clipboard",
      });
    } catch (err) {
      toast({
        description: "Failed to copy to clipboard",
        variant: "destructive",
      });
    }
  };

  return (
    <Card className="shadow-sm transition-all duration-200 animate-fade-in glass-panel h-full flex flex-col" data-testid="chatbot-panel">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg font-medium flex items-center gap-2 flex-wrap">
            <span className="flex items-center">
              <Bot className="h-5 w-5 mr-2 text-primary" />
              Email Assistant
            </span>
            {/* Account context badge */}
            {accountId ? (
              <Badge variant="default" className="text-xs font-normal flex items-center gap-1">
                <CheckCircle className="h-3 w-3" />
                {accountId}
              </Badge>
            ) : (
              <Badge variant="secondary" className="text-xs font-normal flex items-center gap-1">
                <AlertCircle className="h-3 w-3" />
                No Account
              </Badge>
            )}
            {/* Folder context badge */}
            {currentFolder && (
              <Badge variant="outline" className="text-xs font-normal flex items-center gap-1">
                <Folder className="h-3 w-3" />
                {currentFolder}
              </Badge>
            )}
          </CardTitle>

          <div className="flex items-center gap-2">
            {/* Debug toggle */}
            <Button
              variant={debugMode ? "default" : "ghost"}
              size="sm"
              className="h-7 px-2"
              onClick={() => setDebugMode(!debugMode)}
            >
              <Bug className="h-4 w-4" />
            </Button>

            {messages.length > 0 && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="sm" className="h-7 w-7 p-0">
                    <MoreHorizontal className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={handleExportText}>
                    <Download className="mr-2 h-4 w-4" />
                    Export as Text
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleExportJSON}>
                    <Download className="mr-2 h-4 w-4" />
                    Export as JSON
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleCopyToClipboard}>
                    <Copy className="mr-2 h-4 w-4" />
                    Copy to Clipboard
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleClearConversation} className="text-destructive">
                    <Trash2 className="mr-2 h-4 w-4" />
                    Clear Conversation
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </div>
        </div>
      </CardHeader>
      
      <CardContent className="p-0 flex-1 flex flex-col min-h-0">
        <div
          className="overflow-y-auto p-4 space-y-4 flex-1 min-h-0"
          data-testid="chatbot-messages"
        >
          {messages.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-center px-4 text-muted-foreground">
              <Bot className="h-12 w-12 mb-3 text-primary/40" />
              <p className="text-sm">Ask about your emails, check specific folders, or find messages from particular senders.</p>
              <div className="flex flex-wrap gap-2 mt-4 justify-center">
                <Badge 
                  variant="outline" 
                  className="cursor-pointer bg-accent/50 hover:bg-accent/80"
                  onClick={() => handleQuickReplyClick("How many unread emails do I have?")}
                >
                  How many unread emails do I have?
                </Badge>
                <Badge 
                  variant="outline" 
                  className="cursor-pointer bg-accent/50 hover:bg-accent/80"
                  onClick={() => handleQuickReplyClick("Show me my 5 most recent emails")}
                >
                  Show recent emails
                </Badge>
                <Badge 
                  variant="outline" 
                  className="cursor-pointer bg-accent/50 hover:bg-accent/80"
                  onClick={() => handleQuickReplyClick("Check my inbox folder")}
                >
                  Check inbox
                </Badge>
              </div>
            </div>
          ) : (
            <>
              {messages.map((message) => (
                <div 
                  key={message.id} 
                  className={`flex ${message.type === 'user' ? 'justify-end' : 'justify-start'}`}
                >
                  <div 
                    className={`
                      max-w-[85%] rounded-lg p-3 
                      ${
                        message.type === 'user' 
                          ? 'bg-primary text-primary-foreground ml-4'
                          : 'bg-card border mr-4'
                      }
                    `}
                    data-testid={`chatbot-message-${message.type}`}
                  >
                    <div className="flex items-center gap-2 mb-1 text-xs opacity-70">
                      {message.type === 'user' ? (
                        <User className="h-3.5 w-3.5" />
                      ) : (
                        <Bot className="h-3.5 w-3.5" />
                      )}
                      <span>
                        {message.type === 'user' ? 'You' : 'AI Assistant'}
                      </span>
                    </div>
                    
                    <div className="text-sm whitespace-pre-line">
                      {message.text}
                    </div>
                    
                    {/* Email data display */}
                    {message.emailData && (
                      <div className="mt-3 pt-3 border-t border-border/50">
                        {message.emailData.messages && message.emailData.messages.length > 0 && (
                          <div className="space-y-2">
                            {message.emailData.messages.map((email, index) => (
                              <div key={email.id} className="text-xs p-2 rounded bg-muted/30 border">
                                <div className="flex items-center gap-1 mb-1">
                                  <Mail className="h-3 w-3 text-primary" />
                                  <span className="font-medium">{email.from}</span>
                                </div>
                                <div className="font-medium">{email.subject}</div>
                                <div className="opacity-70 text-[10px] mt-1">
                                  {new Date(email.date).toLocaleString()}
                                </div>
                              </div>
                            ))}
                          </div>
                        )}
                        
                        {message.emailData.folders && message.emailData.folders.length > 0 && (
                          <div className="grid grid-cols-2 gap-2">
                            {message.emailData.folders.map((folder) => (
                              <div key={folder.name} className="text-xs p-2 rounded bg-muted/30 border flex items-center">
                                <Folder className="h-3 w-3 text-primary mr-1" />
                                <span>
                                  <span className="font-medium">{folder.name}</span>
                                  <span className="ml-1">({folder.unreadCount}/{folder.count})</span>
                                </span>
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    )}
                    
                    {/* Follow-up suggestions */}
                    {message.followupSuggestions && message.followupSuggestions.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        {message.followupSuggestions.map((suggestion, index) => (
                          <Badge 
                            key={index}
                            variant="outline" 
                            className="cursor-pointer bg-accent/50 hover:bg-accent/80 text-[10px]"
                            onClick={() => handleQuickReplyClick(suggestion)}
                          >
                            {suggestion}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              ))}
              
              {/* Typing indicator when loading */}
              {(chatbotMutation.isPending || (isStreaming && !streamingMessage?.text)) && (
                <div className="flex justify-start">
                  <div className="bg-card border rounded-lg p-3 max-w-[85%] mr-4">
                    <div className="flex items-center gap-2">
                      <Bot className="h-4 w-4" />
                      <div className="flex space-x-1">
                        <div className="h-2 w-2 rounded-full bg-primary animate-pulse" style={{ animationDelay: '0s' }}></div>
                        <div className="h-2 w-2 rounded-full bg-primary animate-pulse" style={{ animationDelay: '0.2s' }}></div>
                        <div className="h-2 w-2 rounded-full bg-primary animate-pulse" style={{ animationDelay: '0.4s' }}></div>
                      </div>
                    </div>
                  </div>
                </div>
              )}
              
              <div ref={messagesEndRef} />
            </>
          )}
        </div>

        {/* Debug Panel */}
        {debugMode && debugInfo && (
          <Collapsible open={debugPanelOpen} onOpenChange={setDebugPanelOpen} className="border-t">
            <CollapsibleTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="w-full h-8 flex items-center justify-between px-4 hover:bg-accent/50"
              >
                <div className="flex items-center gap-2 text-xs">
                  <Bug className="h-3.5 w-3.5" />
                  <span className="font-medium">Debug Information</span>
                  {debugInfo.error && (
                    <XCircle className="h-3.5 w-3.5 text-destructive" />
                  )}
                  {!debugInfo.error && debugInfo.response && (
                    <CheckCircle className="h-3.5 w-3.5 text-green-500" />
                  )}
                  {!debugInfo.error && !debugInfo.response && (
                    <AlertCircle className="h-3.5 w-3.5 text-yellow-500" />
                  )}
                </div>
                {debugPanelOpen ? (
                  <ChevronUp className="h-3.5 w-3.5" />
                ) : (
                  <ChevronDown className="h-3.5 w-3.5" />
                )}
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent className="px-4 pb-3 max-h-60 overflow-y-auto bg-muted/30">
              <div className="space-y-3 text-xs">
                {/* Timestamp */}
                <div className="flex items-center gap-2 pt-2 pb-1 border-b">
                  <span className="font-medium text-muted-foreground">Timestamp:</span>
                  <span className="font-mono">{new Date(debugInfo.timestamp).toLocaleString()}</span>
                </div>

                {/* Request */}
                <div>
                  <div className="font-medium mb-1 flex items-center gap-2">
                    <span>Request:</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-5 px-1"
                      onClick={() => {
                        navigator.clipboard.writeText(JSON.stringify(debugInfo.request, null, 2));
                        toast({ description: "Request copied to clipboard" });
                      }}
                    >
                      <Copy className="h-3 w-3" />
                    </Button>
                  </div>
                  <pre className="bg-card border rounded p-2 overflow-x-auto text-[10px] font-mono">
                    {JSON.stringify(debugInfo.request, null, 2)}
                  </pre>
                </div>

                {/* Response */}
                {debugInfo.response && (
                  <div>
                    <div className="font-medium mb-1 flex items-center gap-2">
                      <CheckCircle className="h-3 w-3 text-green-500" />
                      <span>Response:</span>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-5 px-1"
                        onClick={() => {
                          navigator.clipboard.writeText(JSON.stringify(debugInfo.response, null, 2));
                          toast({ description: "Response copied to clipboard" });
                        }}
                      >
                        <Copy className="h-3 w-3" />
                      </Button>
                    </div>
                    <pre className="bg-card border rounded p-2 overflow-x-auto text-[10px] font-mono">
                      {JSON.stringify(debugInfo.response, null, 2)}
                    </pre>
                  </div>
                )}

                {/* Error */}
                {debugInfo.error && (
                  <div>
                    <div className="font-medium mb-1 flex items-center gap-2 text-destructive">
                      <XCircle className="h-3 w-3" />
                      <span>Error:</span>
                    </div>
                    <div className="bg-destructive/10 border border-destructive/30 rounded p-2 text-destructive">
                      {debugInfo.error}
                    </div>
                  </div>
                )}
              </div>
            </CollapsibleContent>
          </Collapsible>
        )}

        <Separator />

        <form onSubmit={handleSubmit} className="p-4 flex gap-2 flex-shrink-0" data-testid="chatbot-form">
          <Input
            ref={inputRef}
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            placeholder="Type your message..."
            className="flex-1"
            disabled={chatbotMutation.isPending}
            data-testid="chatbot-input"
          />
          <Button
            type="submit"
            disabled={!inputText.trim() || chatbotMutation.isPending || isStreaming}
            data-testid="chatbot-send-button"
          >
            {(chatbotMutation.isPending || isStreaming) ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Send className="h-4 w-4" />
            )}
            <span className="ml-2 hidden sm:inline">Send</span>
          </Button>
        </form>
      </CardContent>
    </Card>
  );
};

export default ChatbotPanel;
