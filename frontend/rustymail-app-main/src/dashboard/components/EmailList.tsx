import React, { useState, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { API_BASE_URL } from '../../config/api';
import { config } from '../config';
import { Card, CardHeader, CardTitle, CardContent } from '../../components/ui/card';
import { useAccount } from '../../contexts/AccountContext';
import { Button } from '../../components/ui/button';
import { Badge } from '../../components/ui/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { RefreshCw, Mail, ChevronLeft, ChevronRight, X, ChevronsLeft, ChevronsRight, Paperclip, Download } from 'lucide-react';
import { format } from 'date-fns';
import { useToast } from '../../hooks/use-toast';
import type { AttachmentInfo, ListAttachmentsResponse } from '../../types';

interface Email {
  id: number;
  uid: number;
  message_id: string | null;
  subject: string | null;
  from_address: string | null;
  from_name: string | null;
  to_addresses: string[];
  date: string | null;
  internal_date: string | null;
  flags: string[];
  body_text: string | null;
}

interface EmailListResponse {
  emails: Email[];
  folder: string;
  count: number;
}

export interface EmailContext {
  uid: number;
  message_id: string | null;
  index: number;
}

interface EmailListProps {
  currentFolder: string;
  setCurrentFolder: (folder: string) => void;
  onEmailSelect?: (context: EmailContext | undefined) => void;
}

const EmailList: React.FC<EmailListProps> = ({ currentFolder, setCurrentFolder, onEmailSelect }) => {
  const { currentAccount } = useAccount();
  const { toast } = useToast();
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
  const [hasAutoSynced, setHasAutoSynced] = useState<Set<string>>(new Set());
  const [attachments, setAttachments] = useState<AttachmentInfo[]>([]);
  const [currentMessageId, setCurrentMessageId] = useState<string>('');
  const [loadingAttachments, setLoadingAttachments] = useState(false);
  const pageSize = 20;

  // Reset to page 1 when folder or account changes
  useEffect(() => {
    setCurrentPage(1);
    setSelectedEmail(null);
    onEmailSelect?.(undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentFolder, currentAccount?.id]);

  const { data, isLoading, error, refetch, isFetching } = useQuery<EmailListResponse>({
    queryKey: ['emails', currentAccount?.id, currentFolder, currentPage],
    queryFn: async () => {
      if (!currentAccount) {
        throw new Error('No account selected');
      }
      const offset = (currentPage - 1) * pageSize;
      const response = await fetch(
        `${API_BASE_URL}/dashboard/emails?account_id=${currentAccount.id}&folder=${encodeURIComponent(currentFolder)}&limit=${pageSize}&offset=${offset}`,
        {
          headers: {
            'X-API-Key': config.api.apiKey
          }
        }
      );
      if (!response.ok) {
        throw new Error('Failed to fetch emails');
      }
      return response.json();
    },
    enabled: !!currentAccount,
    refetchInterval: 30000, // Refetch every 30 seconds
  });

  const handleSync = async () => {
    if (!currentAccount) {
      toast({
        title: "No Account Selected",
        description: "Please select an account before syncing.",
        variant: "destructive",
      });
      return;
    }

    try {
      toast({
        title: "Syncing...",
        description: `Starting sync for ${currentAccount.email_address}`,
      });

      const response = await fetch(`${API_BASE_URL}/dashboard/sync/trigger?account_id=${currentAccount.id}`, {
        method: 'POST',
        headers: {
          'X-API-Key': config.api.apiKey
        }
      });

      if (response.ok) {
        toast({
          title: "Sync Started",
          description: "Email sync is running in the background. This may take a few moments.",
        });
        // Wait a moment then refetch
        setTimeout(() => refetch(), 2000);
      } else {
        const errorText = await response.text();
        toast({
          title: "Sync Failed",
          description: errorText || "Failed to start email sync",
          variant: "destructive",
        });
      }
    } catch (error) {
      console.error('Failed to trigger sync:', error);
      toast({
        title: "Sync Error",
        description: error instanceof Error ? error.message : "An unknown error occurred",
        variant: "destructive",
      });
    }
  };

  const fetchAttachments = async (email: Email) => {
    if (!currentAccount) return;

    setLoadingAttachments(true);
    try {
      const response = await fetch(
        `${API_BASE_URL}/dashboard/attachments/list?account_id=${currentAccount.id}&folder=${encodeURIComponent(currentFolder)}&uid=${email.uid}`,
        {
          headers: {
            'X-API-Key': config.api.apiKey
          }
        }
      );

      if (response.ok) {
        const data: ListAttachmentsResponse = await response.json();
        setAttachments(data.attachments);
        setCurrentMessageId(data.message_id);
      } else {
        console.error('Failed to fetch attachments');
        setAttachments([]);
        setCurrentMessageId('');
      }
    } catch (error) {
      console.error('Error fetching attachments:', error);
      setAttachments([]);
    } finally {
      setLoadingAttachments(false);
    }
  };

  const downloadAttachment = async (messageId: string, filename: string) => {
    if (!currentAccount) return;

    try {
      const url = `${API_BASE_URL}/dashboard/attachments/${encodeURIComponent(messageId)}/${encodeURIComponent(filename)}?account_id=${currentAccount.id}`;
      window.open(url, '_blank');
    } catch (error) {
      console.error('Error downloading attachment:', error);
      toast({
        title: "Download Failed",
        description: "Failed to download attachment",
        variant: "destructive",
      });
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${Math.round((bytes / Math.pow(k, i)) * 100) / 100} ${sizes[i]}`;
  };

  // Fetch attachments when email is selected
  useEffect(() => {
    if (selectedEmail) {
      fetchAttachments(selectedEmail);
    } else {
      setAttachments([]);
      setCurrentMessageId('');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedEmail]);

  // Auto-sync when cache is empty for the current account/folder combination
  useEffect(() => {
    if (!currentAccount) return;

    // Create unique key for this account+folder combination
    const cacheKey = `${currentAccount.id}:${currentFolder}`;

    // Only auto-sync if:
    // 1. We have data loaded
    // 2. The cache is empty (0 emails)
    // 3. Not currently fetching
    // 4. On first page
    // 5. Haven't already auto-synced for this account+folder combination
    if (data && data.emails.length === 0 && !isFetching && currentPage === 1 && !hasAutoSynced.has(cacheKey)) {
      console.log(`Cache is empty for account ${currentAccount.email_address} folder ${currentFolder}, triggering automatic sync...`);

      // Mark this account+folder as auto-synced
      setHasAutoSynced(prev => new Set(prev).add(cacheKey));

      // Trigger the sync
      handleSync();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, isFetching, currentPage, currentAccount?.id, currentFolder]);

  const totalPages = Math.ceil((data?.count || 0) / pageSize);

  const formatDate = (dateStr: string | null, internalDateStr: string | null) => {
    const dateToUse = dateStr || internalDateStr;
    if (!dateToUse) return 'No date';
    try {
      return format(new Date(dateToUse), 'MMM d, yyyy HH:mm');
    } catch {
      return dateToUse;
    }
  };

  const truncateText = (text: string | null, maxLength: number) => {
    if (!text) return '';
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + '...';
  };

  // Handle ESC key to close modal
  useEffect(() => {
    const handleEsc = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setSelectedEmail(null);
        onEmailSelect?.(undefined);
      }
    };

    if (selectedEmail) {
      document.addEventListener('keydown', handleEsc);
      return () => document.removeEventListener('keydown', handleEsc);
    }
  }, [selectedEmail]);

  if (!currentAccount) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email List
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">No account selected. Please select an account to view emails.</p>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email List
          </CardTitle>
        </CardHeader>
        <CardContent className="p-6">
          <p className="text-red-500">Error loading emails: {(error as Error).message}</p>
          <Button onClick={() => refetch()} className="mt-4">
            <RefreshCw className="mr-2 h-4 w-4" />
            Retry
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col min-h-0">
      <CardHeader className="flex flex-row items-center justify-between flex-shrink-0">
        <CardTitle className="flex items-center gap-3">
          <Mail className="h-5 w-5" />
          <Select value={currentFolder} onValueChange={setCurrentFolder}>
            <SelectTrigger className="w-[180px] h-8">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="INBOX">Inbox</SelectItem>
              <SelectItem value="INBOX.Sent">Sent</SelectItem>
              <SelectItem value="INBOX.Drafts">Drafts</SelectItem>
              <SelectItem value="INBOX.Trash">Trash</SelectItem>
              <SelectItem value="INBOX.spam">Spam</SelectItem>
            </SelectContent>
          </Select>
          <span className="text-sm font-normal text-muted-foreground">
            ({data?.emails.length || 0} of {data?.count || 0} emails)
          </span>
        </CardTitle>
        <div className="flex gap-2">
          <Button
            onClick={handleSync}
            disabled={isFetching}
            size="sm"
            variant="outline"
          >
            <RefreshCw className={`mr-2 h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
            Sync
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex-1 flex flex-col overflow-hidden">
        {isLoading ? (
          <div className="flex items-center justify-center p-8">
            <RefreshCw className="h-8 w-8 animate-spin text-gray-400" />
          </div>
        ) : (
          <>
            <div className="flex-1 overflow-y-auto min-h-0">
              <div className="space-y-2">
                {data?.emails.map((email, arrayIndex) => {
                  const offset = (currentPage - 1) * pageSize;
                  const emailIndex = offset + arrayIndex;
                  return (
                  <div
                    key={email.id}
                    className="border rounded-lg p-3 hover:bg-gray-50 cursor-pointer transition-colors"
                    onMouseEnter={() => onEmailSelect?.({ uid: email.uid, message_id: email.message_id, index: emailIndex })}
                    onClick={() => {
                      setSelectedEmail(email);
                      onEmailSelect?.({ uid: email.uid, message_id: email.message_id, index: emailIndex });
                    }}
                  >
                    <div className="flex justify-between items-start mb-1">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm">
                            {email.from_name || email.from_address || 'Unknown sender'}
                          </span>
                          {email.flags.includes('\\Seen') ? null : (
                            <Badge variant="default" className="text-xs">Unread</Badge>
                          )}
                        </div>
                        <div className="text-sm font-semibold mt-1">
                          {email.subject || '(No subject)'}
                        </div>
                        {email.body_text && (
                          <div className="text-xs text-gray-600 mt-1">
                            {truncateText(email.body_text, 100)}
                          </div>
                        )}
                      </div>
                      <div className="text-xs text-gray-500">
                        {formatDate(email.date, email.internal_date)}
                      </div>
                    </div>
                  </div>
                  );
                })}
              </div>
            </div>

            {/* Pagination */}
            <div className="flex items-center justify-between mt-4 pt-4 border-t flex-shrink-0">
              <div className="flex gap-1">
                <Button
                  onClick={() => setCurrentPage(1)}
                  disabled={currentPage === 1}
                  size="sm"
                  variant="outline"
                  title="First page"
                >
                  <ChevronsLeft className="h-4 w-4" />
                </Button>
                <Button
                  onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                  disabled={currentPage === 1}
                  size="sm"
                  variant="outline"
                >
                  <ChevronLeft className="h-4 w-4 mr-1" />
                  Previous
                </Button>
              </div>
              <span className="text-sm text-gray-600">
                Page {currentPage} of {totalPages}
              </span>
              <div className="flex gap-1">
                <Button
                  onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                  disabled={currentPage >= totalPages}
                  size="sm"
                  variant="outline"
                >
                  Next
                  <ChevronRight className="h-4 w-4 ml-1" />
                </Button>
                <Button
                  onClick={() => setCurrentPage(totalPages)}
                  disabled={currentPage >= totalPages}
                  size="sm"
                  variant="outline"
                  title="Last page"
                >
                  <ChevronsRight className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </>
        )}

        {/* Email Preview Modal */}
        {selectedEmail && (
          <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            <div className="bg-white rounded-lg p-6 max-w-2xl w-full max-h-[80vh] overflow-y-auto relative">
              {/* Close button in top-right corner */}
              <button
                onClick={() => {
                  setSelectedEmail(null);
                  onEmailSelect?.(undefined);
                }}
                className="absolute top-4 right-4 p-1 hover:bg-gray-100 rounded-full transition-colors"
                aria-label="Close"
              >
                <X className="h-5 w-5 text-gray-500" />
              </button>

              <div className="mb-4 pr-8">
                <h3 className="text-lg font-semibold">{selectedEmail.subject || '(No subject)'}</h3>
                <p className="text-sm text-gray-600">
                  From: {selectedEmail.from_name || selectedEmail.from_address || 'Unknown'}
                </p>
                <p className="text-sm text-gray-600">
                  Date: {formatDate(selectedEmail.date, selectedEmail.internal_date)}
                </p>
              </div>
              <div className="mb-4 whitespace-pre-wrap">
                {selectedEmail.body_text || 'No content'}
              </div>

              {/* Attachments Section */}
              {loadingAttachments ? (
                <div className="mb-4 flex items-center gap-2 text-sm text-gray-600">
                  <RefreshCw className="h-4 w-4 animate-spin" />
                  Loading attachments...
                </div>
              ) : attachments.length > 0 ? (
                <div className="mb-4">
                  <h4 className="text-sm font-semibold mb-2 flex items-center gap-2">
                    <Paperclip className="h-4 w-4" />
                    Attachments ({attachments.length})
                  </h4>
                  <div className="space-y-2">
                    {attachments.map((attachment, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between p-2 bg-gray-50 rounded border"
                      >
                        <div className="flex items-center gap-2 flex-1 min-w-0">
                          <Paperclip className="h-4 w-4 text-gray-500 flex-shrink-0" />
                          <div className="flex-1 min-w-0">
                            <div className="text-sm font-medium truncate">
                              {attachment.filename}
                            </div>
                            <div className="text-xs text-gray-500">
                              {formatFileSize(attachment.size_bytes)}
                              {attachment.content_type && ` • ${attachment.content_type}`}
                            </div>
                          </div>
                        </div>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => downloadAttachment(currentMessageId, attachment.filename)}
                        >
                          <Download className="h-4 w-4" />
                        </Button>
                      </div>
                    ))}
                  </div>
                </div>
              ) : null}

              <Button onClick={() => {
                setSelectedEmail(null);
                onEmailSelect?.(undefined);
              }}>Close</Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

export default EmailList;