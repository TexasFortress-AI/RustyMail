
import React, { useState, useRef, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { useChatbotMutation } from '@/dashboard/api/hooks';
import { ChatMessage } from '@/types';
import { Send, Bot, User, Loader2, Mail, Folder } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { toast } from '@/hooks/use-toast';

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

const ChatbotPanel: React.FC = () => {
  const [inputText, setInputText] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>(loadConversation);
  const [conversationId, setConversationId] = useState<string | undefined>(undefined);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  
  const chatbotMutation = useChatbotMutation();
  
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
  
  // Handle form submission
  const handleSubmit = (e: React.FormEvent) => {
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
    setInputText('');
    
    // Send the query to the chatbot
    chatbotMutation.mutate(
      { 
        query: inputText, 
        conversation_id: conversationId 
      },
      {
        onSuccess: (response) => {
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
  };

  return (
    <Card className="shadow-sm transition-all duration-200 animate-fade-in glass-panel" data-testid="chatbot-panel">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg font-medium flex items-center">
            <span className="flex items-center">
              <Bot className="h-5 w-5 mr-2 text-primary" />
              Email Assistant
            </span>
          </CardTitle>
          
          {messages.length > 0 && (
            <Button 
              variant="ghost" 
              size="sm" 
              onClick={handleClearConversation}
              className="text-xs h-7"
            >
              Clear conversation
            </Button>
          )}
        </div>
      </CardHeader>
      
      <CardContent className="p-0">
        <div className="h-64 md:h-80 overflow-y-auto p-4 space-y-4" data-testid="chatbot-messages">
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
              {chatbotMutation.isPending && (
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
        
        <Separator />
        
        <form onSubmit={handleSubmit} className="p-4 flex gap-2" data-testid="chatbot-form">
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
            disabled={!inputText.trim() || chatbotMutation.isPending}
            data-testid="chatbot-send-button"
          >
            {chatbotMutation.isPending ? (
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
