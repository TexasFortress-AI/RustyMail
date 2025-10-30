// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

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
import { RefreshCw, Mail, ChevronLeft, ChevronRight, X, ChevronsLeft, ChevronsRight, Paperclip, Download, PenSquare, Trash2, FolderOpen } from 'lucide-react';
import { format } from 'date-fns';
import { useToast } from '../../hooks/use-toast';
import type { AttachmentInfo, ListAttachmentsResponse } from '../../types';
import { SendMailDialog } from './SendMailDialog';

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
  has_attachments: boolean;
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
  onRefetchReady?: (refetch: () => void) => void;
  onComposeRequest?: (handler: (mode: 'reply' | 'forward', originalEmail: Email) => void) => void;
}

const EmailList: React.FC<EmailListProps> = ({ currentFolder, setCurrentFolder, onEmailSelect, onRefetchReady, onComposeRequest }) => {
  const { currentAccount } = useAccount();
  const { toast } = useToast();
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
  const [hasAutoSynced, setHasAutoSynced] = useState<Set<string>>(new Set());
  const [attachments, setAttachments] = useState<AttachmentInfo[]>([]);
  const [currentMessageId, setCurrentMessageId] = useState<string>('');
  const [loadingAttachments, setLoadingAttachments] = useState(false);
  const [composeDialogOpen, setComposeDialogOpen] = useState(false);
  const [hasUserInteracted, setHasUserInteracted] = useState(false);

  // Add a mounted flag to prevent any dialog operations until fully mounted
  const [isMounted, setIsMounted] = useState(false);

  // Add an additional safety flag that absolutely prevents dialog rendering
  const [dialogEnabled, setDialogEnabled] = useState(false);

  // Debug: Component mount - delay mounted flag to avoid any initial render issues
  useEffect(() => {
    console.log('[EmailList] Component mounted, delaying mounted flag...');
    const timer = setTimeout(() => {
      console.log('[EmailList] Setting isMounted to true');
      setIsMounted(true);
    }, 500); // Half second delay to ensure everything is settled
    return () => {
      console.log('[EmailList] Component unmounting');
      clearTimeout(timer);
    };
  }, []);

  // Enable dialog only after user interaction AND mount
  useEffect(() => {
    if (isMounted && hasUserInteracted) {
      console.log('[EmailList] Enabling dialog - user has interacted and component is mounted');
      setDialogEnabled(true);
    }
  }, [isMounted, hasUserInteracted]);

  // Debug: Log when dialog state changes with stack trace
  useEffect(() => {
    console.log('[EmailList] Compose dialog state changed:', composeDialogOpen, 'dialogEnabled:', dialogEnabled);
    if (composeDialogOpen && !dialogEnabled) {
      console.error('[EmailList] CRITICAL: Dialog trying to open but not enabled! Forcing closed.');
      setComposeDialogOpen(false);
    }
    if (composeDialogOpen) {
      console.trace('[EmailList] Dialog opened - stack trace:');
    }
  }, [composeDialogOpen, dialogEnabled]);
  const [composeMode, setComposeMode] = useState<'compose' | 'reply' | 'forward'>('compose');
  const [composeOriginalEmail, setComposeOriginalEmail] = useState<Email | null>(null);
  const [folderMovePopup, setFolderMovePopup] = useState<{email: Email, x: number, y: number} | null>(null);
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
    // Poll every 5 seconds for Outbox (to catch rapid changes), 30 seconds for other folders
    refetchInterval: currentFolder === 'INBOX.Outbox' ? 5000 : 30000,
  });

  // Fetch available folders from IMAP
  const { data: foldersData, isLoading: foldersLoading, error: foldersError } = useQuery<{account_id: string; folders: string[]}>({
    queryKey: ['folders', currentAccount?.id],
    queryFn: async () => {
      if (!currentAccount) {
        throw new Error('No account selected');
      }

      // Add timeout to prevent hanging
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000); // 5 second timeout

      try {
        const response = await fetch(
          `${API_BASE_URL}/dashboard/folders?account_id=${currentAccount.id}`,
          {
            headers: {
              'X-API-Key': config.api.apiKey
            },
            signal: controller.signal
          }
        );
        clearTimeout(timeoutId);

        if (!response.ok) {
          throw new Error('Failed to fetch folders');
        }
        return response.json();
      } catch (error) {
        clearTimeout(timeoutId);
        if (error instanceof Error && error.name === 'AbortError') {
          console.warn('Folders fetch timed out, using fallback');
          // Return fallback folders instead of throwing
          return { account_id: currentAccount.id, folders: ['INBOX', 'INBOX.Sent', 'INBOX.Drafts', 'INBOX.Trash'] };
        }
        throw error;
      }
    },
    enabled: !!currentAccount,
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
    retry: false, // Don't retry on failure
  });

  // Handle compose requests from EmailBody
  const handleComposeRequest = (mode: 'reply' | 'forward', originalEmail: Email) => {
    console.log('[EmailList] handleComposeRequest called:', mode);
    setComposeMode(mode);
    setComposeOriginalEmail(originalEmail);
    setHasUserInteracted(true);
    setComposeDialogOpen(true);
  };

  // Expose compose handler to parent
  useEffect(() => {
    console.log('[EmailList] useEffect for onComposeRequest running');
    if (onComposeRequest) {
      console.log('[EmailList] Registering handleComposeRequest with parent');
      onComposeRequest(handleComposeRequest);
    }
  }, [onComposeRequest]);

  // Expose refetch function to parent
  useEffect(() => {
    if (onRefetchReady) {
      onRefetchReady(() => refetch());
    }
  }, [refetch, onRefetchReady]);

  // Validate currentFolder exists when folders data loads or account changes
  useEffect(() => {
    // Only validate if we have folders data and a current folder
    if (foldersData?.folders && foldersData.folders.length > 0 && currentFolder) {
      // If current folder doesn't exist for this account, switch to INBOX
      if (!foldersData.folders.includes(currentFolder)) {
        console.log(`Folder ${currentFolder} doesn't exist for account ${currentAccount?.email_address}, switching to INBOX`);
        setCurrentFolder('INBOX');
      }
    }
    // Only run when account ID or folders data changes, NOT when currentFolder changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentAccount?.id, foldersData]);

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

  const handleDeleteEmail = async (email: Email, event: React.MouseEvent) => {
    // Prevent the click from triggering email selection
    event.stopPropagation();

    if (!currentAccount) return;

    if (!window.confirm(`Are you sure you want to delete "${email.subject || '(No subject)'}"`)) {
      return;
    }

    try {
      const response = await fetch(`${API_BASE_URL}/dashboard/emails/delete`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-API-Key': config.api.apiKey
        },
        body: JSON.stringify({
          folder: currentFolder,
          uids: [email.uid],
          account_email: currentAccount.email_address
        })
      });

      if (response.ok) {
        toast({
          title: "Email Deleted",
          description: "The email has been deleted successfully.",
        });
        // Refetch emails to update the list
        refetch();
        // Clear selected email if it was deleted
        if (selectedEmail?.uid === email.uid) {
          setSelectedEmail(null);
          onEmailSelect?.(undefined);
        }
      } else {
        const errorData = await response.json();
        toast({
          title: "Delete Failed",
          description: errorData.error || "Failed to delete email",
          variant: "destructive",
        });
      }
    } catch (error) {
      console.error('Error deleting email:', error);
      toast({
        title: "Delete Error",
        description: error instanceof Error ? error.message : "An unknown error occurred",
        variant: "destructive",
      });
    }
  };

  const handleMoveClick = (email: Email, event: React.MouseEvent) => {
    event.stopPropagation();
    const rect = (event.target as HTMLElement).getBoundingClientRect();
    setFolderMovePopup({
      email,
      x: rect.left,
      y: rect.bottom + 4
    });
  };

  const handleMoveToFolder = async (email: Email, targetFolder: string) => {
    setFolderMovePopup(null); // Close popup immediately

    if (!currentAccount) {
      toast({
        title: "Error",
        description: "No account selected",
        variant: "destructive",
      });
      return;
    }

    if (targetFolder === currentFolder) {
      toast({
        title: "Notice",
        description: "Email is already in this folder",
      });
      return;
    }

    try {
      const response = await fetch(`${API_BASE_URL}/dashboard/emails/move`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-API-Key': config.api.apiKey
        },
        body: JSON.stringify({
          account_id: currentAccount.id,
          uid: email.uid,
          from_folder: currentFolder,
          to_folder: targetFolder
        })
      });

      const data = await response.json();

      if (response.ok) {
        toast({
          title: "Success",
          description: `Email moved to ${targetFolder}`,
        });
        // Refetch to update the list
        refetch();
      } else {
        const errorData = await response.json().catch(() => ({ error: 'Failed to parse error' }));
        toast({
          title: "Error",
          description: errorData.error || "Failed to move email",
          variant: "destructive",
        });
      }
    } catch (error) {
      console.error('Error moving email:', error);
      toast({
        title: "Error",
        description: error instanceof Error ? error.message : "An unknown error occurred",
        variant: "destructive",
      });
    }
  };

  // Close folder popup when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (folderMovePopup) {
        setFolderMovePopup(null);
      }
    };

    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  }, [folderMovePopup]);

  // Close folder popup on ESC
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && folderMovePopup) {
        setFolderMovePopup(null);
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [folderMovePopup]);

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

  const formatFolderName = (folder: string): string => {
    // Special case for INBOX
    if (folder === 'INBOX') return 'Inbox';
    // Remove INBOX. prefix if present
    const name = folder.replace('INBOX.', '');
    // Capitalize first letter
    return name.charAt(0).toUpperCase() + name.slice(1);
  };

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
    <>
    <Card className="h-full flex flex-col min-h-0">
      <CardHeader className="flex flex-row items-center justify-between flex-shrink-0">
        <CardTitle className="flex items-center gap-3">
          <Mail className="h-5 w-5" />
          <Select value={currentFolder} onValueChange={setCurrentFolder}>
            <SelectTrigger className="w-[180px] h-8">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {foldersLoading ? (
                <SelectItem value={currentFolder} disabled>Loading folders...</SelectItem>
              ) : (foldersData?.folders && foldersData.folders.length > 0) ? (
                foldersData.folders.map((folder) => (
                  <SelectItem key={folder} value={folder}>
                    {formatFolderName(folder)}
                  </SelectItem>
                ))
              ) : (
                <SelectItem value="INBOX">Inbox</SelectItem>
              )}
            </SelectContent>
          </Select>
          <span className="text-sm font-normal text-muted-foreground">
            ({data?.emails.length || 0} of {data?.count || 0} emails)
          </span>
        </CardTitle>
        <div className="flex gap-2">
          <Button
            onClick={() => {
              console.log('[EmailList] Compose button clicked');
              setHasUserInteracted(true);
              setComposeDialogOpen(true);
            }}
            size="sm"
            variant="default"
          >
            <PenSquare className="mr-2 h-4 w-4" />
            Compose
          </Button>
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
                    className="group border rounded-lg p-3 hover:bg-gray-50 cursor-pointer transition-colors"
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
                            {(currentFolder === 'INBOX.Sent' || currentFolder === 'INBOX.Outbox' || currentFolder === 'INBOX.Drafts')
                              ? (email.to_addresses && email.to_addresses.length > 0
                                  ? email.to_addresses.join(', ')
                                  : 'No recipients')
                              : (email.from_name || email.from_address || 'Unknown sender')}
                          </span>
                          {email.flags.includes('\\Seen') ? null : (
                            <Badge variant="default" className="text-xs">Unread</Badge>
                          )}
                        </div>
                        <div className="text-sm font-semibold mt-1 flex items-center gap-1">
                          {email.has_attachments && (
                            <Paperclip className="h-3 w-3 text-muted-foreground flex-shrink-0" />
                          )}
                          <span>{email.subject || '(No subject)'}</span>
                        </div>
                        {email.body_text && (
                          <div className="text-xs text-gray-600 mt-1">
                            {truncateText(email.body_text, 100)}
                          </div>
                        )}
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="text-xs text-gray-500">
                          {formatDate(email.date, email.internal_date)}
                        </div>
                        <button
                          onClick={(e) => handleMoveClick(email, e)}
                          className="opacity-0 group-hover:opacity-100 hover:bg-blue-100 p-1 rounded transition-all"
                          title="Move to folder"
                        >
                          <FolderOpen className="h-4 w-4 text-blue-600" />
                        </button>
                        <button
                          onClick={(e) => handleDeleteEmail(email, e)}
                          className="opacity-0 group-hover:opacity-100 hover:bg-red-100 p-1 rounded transition-all"
                          title="Delete email"
                        >
                          <Trash2 className="h-4 w-4 text-red-600" />
                        </button>
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

      {/* Send Mail Dialog - Only render when explicitly enabled by user interaction */}
      {dialogEnabled && (
        <SendMailDialog
          open={composeDialogOpen}
          onOpenChange={(open) => {
            console.log('[EmailList] onOpenChange called with:', open, 'current state:', composeDialogOpen, 'dialogEnabled:', dialogEnabled);
            // Absolutely prevent opening if dialog not enabled
            if (!dialogEnabled) {
              console.error('[EmailList] BLOCKING: Dialog not enabled, cannot change state');
              return;
            }
            // Only allow dialog state changes if user has interacted
            if (!hasUserInteracted && open) {
              console.warn('[EmailList] Blocking dialog open - no user interaction yet');
              return;
            }
            setComposeDialogOpen(open);

            if (!open) {
              // Reset to compose mode when dialog closes
              setComposeMode('compose');
              setComposeOriginalEmail(null);
            }
          }}
          accountEmail={currentAccount?.email_address}
          mode={composeMode}
          originalEmail={composeOriginalEmail || undefined}
          onSuccess={() => {
          // Immediate refetch (email is queued but not in IMAP yet)
          refetch();
          // Delayed refetches to catch the email appearing in Outbox then moving to Sent
          setTimeout(() => refetch(), 2000);  // After worker saves to Outbox
          setTimeout(() => refetch(), 8000);  // After worker moves to Sent
        }}
      />
      )}
    </Card>

    {/* Folder Move Popup */}
    {folderMovePopup && foldersData && (
      <div
        className="absolute z-50 bg-white border rounded-lg shadow-lg py-2"
        style={{
          left: `${folderMovePopup.x}px`,
          top: `${folderMovePopup.y}px`,
          minWidth: '200px'
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-3 py-2 text-sm font-semibold border-b">
          Move to folder:
        </div>
        <div className="max-h-64 overflow-y-auto">
          {foldersData.folders
            .filter(folder => folder !== currentFolder)
            .map(folder => (
              <button
                key={folder}
                onClick={() => handleMoveToFolder(folderMovePopup.email, folder)}
                className="w-full px-3 py-2 text-sm text-left hover:bg-gray-100 transition-colors"
              >
                {folder}
              </button>
            ))}
        </div>
      </div>
    )}
    </>
  );
};

export default EmailList;