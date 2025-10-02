import React, { useState, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { API_BASE_URL } from '../../config/api';
import { config } from '../config';
import { Card, CardHeader, CardTitle, CardContent } from '../../components/ui/card';
import { Button } from '../../components/ui/button';
import { Badge } from '../../components/ui/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { RefreshCw, Mail, ChevronLeft, ChevronRight, X, ChevronsLeft, ChevronsRight } from 'lucide-react';
import { format } from 'date-fns';

interface Email {
  id: number;
  uid: number;
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

const EmailList: React.FC = () => {
  const [currentFolder, setCurrentFolder] = useState('INBOX');
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);
  const pageSize = 20;

  // Reset to page 1 when folder changes
  useEffect(() => {
    setCurrentPage(1);
  }, [currentFolder]);

  const { data, isLoading, error, refetch, isFetching } = useQuery<EmailListResponse>({
    queryKey: ['emails', currentPage],
    queryFn: async () => {
      const offset = (currentPage - 1) * pageSize;
      const response = await fetch(
        `${API_BASE_URL}/dashboard/emails?limit=${pageSize}&offset=${offset}`,
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
    refetchInterval: 30000, // Refetch every 30 seconds
  });

  const handleSync = async () => {
    try {
      const response = await fetch(`${API_BASE_URL}/dashboard/sync/trigger`, {
        method: 'POST',
      });
      if (response.ok) {
        // Wait a moment then refetch
        setTimeout(() => refetch(), 2000);
      }
    } catch (error) {
      console.error('Failed to trigger sync:', error);
    }
  };

  const totalPages = Math.ceil(279 / pageSize); // We know there are 279 emails

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
      }
    };

    if (selectedEmail) {
      document.addEventListener('keydown', handleEsc);
      return () => document.removeEventListener('keydown', handleEsc);
    }
  }, [selectedEmail]);

  if (error) {
    return (
      <Card>
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
              <SelectItem value="Sent">Sent</SelectItem>
              <SelectItem value="Drafts">Drafts</SelectItem>
              <SelectItem value="Trash">Trash</SelectItem>
              <SelectItem value="Spam">Spam</SelectItem>
            </SelectContent>
          </Select>
          <span className="text-sm font-normal text-muted-foreground">
            ({data?.emails.length || 0} of 279 emails)
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
                {data?.emails.map((email) => (
                  <div
                    key={email.id}
                    className="border rounded-lg p-3 hover:bg-gray-50 cursor-pointer transition-colors"
                    onClick={() => setSelectedEmail(email)}
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
                ))}
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
                onClick={() => setSelectedEmail(null)}
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
              <Button onClick={() => setSelectedEmail(null)}>Close</Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

export default EmailList;