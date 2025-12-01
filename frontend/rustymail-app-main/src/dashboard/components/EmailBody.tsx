// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useState, useEffect } from 'react';
import { API_BASE_URL } from '../../config/api';
import { config } from '../config';
import { Card, CardHeader, CardTitle, CardContent } from '../../components/ui/card';
import { Button } from '../../components/ui/button';
import { useAccount } from '../../contexts/AccountContext';
import { RefreshCw, Mail, Paperclip, Download, Archive, Reply, Forward, Code, FileText, Image } from 'lucide-react';
import { format } from 'date-fns';
import { useToast } from '../../hooks/use-toast';
import type { AttachmentInfo, ListAttachmentsResponse } from '../../types';
import { EmailContext } from './EmailList';
import DOMPurify from 'dompurify';
import './EmailBody.css';

interface SanitizeOptions {
  showImages: boolean;
  messageId?: string;
  accountId?: string;
}

// Helper function to sanitize HTML with image handling
const sanitizeEmailHtml = (html: string, options: SanitizeOptions): string => {
  const { showImages, messageId, accountId } = options;

  // Configure DOMPurify hooks for image handling
  DOMPurify.removeAllHooks();

  if (!showImages) {
    // When images are blocked, replace img tags with placeholder text
    DOMPurify.addHook('uponSanitizeElement', (node, data) => {
      if (data.tagName === 'img') {
        const placeholder = document.createElement('span');
        placeholder.className = 'inline-block px-2 py-1 text-xs bg-gray-200 text-gray-600 rounded';
        placeholder.textContent = '[Image blocked]';
        node.parentNode?.replaceChild(placeholder, node);
      }
    });
  } else {
    // When images are shown, handle cid: URIs by rewriting to backend endpoint
    DOMPurify.addHook('afterSanitizeAttributes', (node) => {
      if (node.tagName === 'IMG') {
        const src = node.getAttribute('src') || '';
        if (src.startsWith('cid:')) {
          // Extract the Content-ID (remove 'cid:' prefix)
          const contentId = src.substring(4);

          if (messageId && accountId) {
            // Rewrite to backend inline attachment endpoint
            const inlineUrl = `${API_BASE_URL}/dashboard/attachments/${encodeURIComponent(messageId)}/inline/${encodeURIComponent(contentId)}?account_id=${encodeURIComponent(accountId)}`;
            node.setAttribute('src', inlineUrl);
          } else {
            // Fallback: show placeholder if we don't have message context
            const placeholder = document.createElement('span');
            placeholder.className = 'inline-block px-2 py-1 text-xs bg-yellow-100 text-yellow-800 rounded border border-yellow-300';
            placeholder.textContent = '[Embedded image]';
            node.parentNode?.replaceChild(placeholder, node);
          }
        }
      }
    });
  }

  // Make links open in new tab
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    if (node.tagName === 'A') {
      node.setAttribute('target', '_blank');
      node.setAttribute('rel', 'noopener noreferrer');
    }
  });

  const purifyConfig: DOMPurify.Config = {
    ALLOWED_TAGS: [
      'p', 'br', 'strong', 'em', 'u', 's', 'a', 'ul', 'ol', 'li', 'blockquote',
      'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'table', 'thead',
      'tbody', 'tr', 'th', 'td', 'img', 'hr', 'div', 'span', 'font', 'center', 'b', 'i'
    ],
    ALLOWED_ATTR: showImages
      ? ['href', 'target', 'rel', 'src', 'alt', 'width', 'height', 'style', 'class', 'align', 'valign', 'bgcolor', 'color', 'size', 'face', 'border', 'cellpadding', 'cellspacing']
      : ['href', 'target', 'rel', 'class', 'align', 'valign', 'bgcolor', 'color', 'size', 'face', 'border', 'cellpadding', 'cellspacing'],
    ALLOW_DATA_ATTR: false,
    FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form', 'input', 'style'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick', 'onmouseover'],
  };

  return DOMPurify.sanitize(html, purifyConfig);
};

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
  body_html?: string | null;
}

interface EmailBodyProps {
  currentFolder: string;
  selectedEmailContext: EmailContext | undefined;
  onAttachmentsLoaded?: () => void;
}

const EmailBody: React.FC<EmailBodyProps> = ({ currentFolder, selectedEmailContext, onAttachmentsLoaded }) => {
  const { currentAccount } = useAccount();
  const { toast } = useToast();
  const [email, setEmail] = useState<Email | null>(null);
  const [loading, setLoading] = useState(false);
  const [attachments, setAttachments] = useState<AttachmentInfo[]>([]);
  const [currentMessageId, setCurrentMessageId] = useState<string>('');
  const [loadingAttachments, setLoadingAttachments] = useState(false);
  const [viewMode, setViewMode] = useState<'html' | 'text'>('html');
  const [showImages, setShowImages] = useState(false);

  useEffect(() => {
    setShowImages(false);
    const fetchEmail = async () => {
      if (!currentAccount || !selectedEmailContext) {
        setEmail(null);
        setAttachments([]);
        return;
      }

      setLoading(true);
      try {
        // Fetch single email by folder and UID
        const response = await fetch(
          `${API_BASE_URL}/dashboard/emails?account_id=${currentAccount.id}&folder=${encodeURIComponent(currentFolder)}&limit=1&offset=${selectedEmailContext.index}`,
          {
            headers: {
              'X-API-Key': config.api.apiKey
            }
          }
        );

        if (response.ok) {
          const data = await response.json();
          if (data.emails && data.emails.length > 0) {
            setEmail(data.emails[0]);
          } else {
            setEmail(null);
          }
        } else {
          console.error('Failed to fetch email');
          setEmail(null);
        }
      } catch (error) {
        console.error('Error fetching email:', error);
        setEmail(null);
      } finally {
        setLoading(false);
      }
    };

    fetchEmail();
  }, [currentAccount, selectedEmailContext, currentFolder]);

  // Fetch attachments when email is loaded
  useEffect(() => {
    const fetchAttachments = async () => {
      if (!currentAccount || !email) {
        setAttachments([]);
        setCurrentMessageId('');
        return;
      }

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

          // Notify parent that attachments were loaded so email list can update
          if (data.attachments.length > 0 && onAttachmentsLoaded) {
            onAttachmentsLoaded();
          }
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

    fetchAttachments();
  }, [email, currentAccount, currentFolder]);

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

  const downloadAllAttachments = async () => {
    if (!currentAccount || !currentMessageId || attachments.length === 0) return;

    try {
      const url = `${API_BASE_URL}/dashboard/attachments/${encodeURIComponent(currentMessageId)}/zip?account_id=${currentAccount.id}`;
      window.open(url, '_blank');

      toast({
        title: "Download Started",
        description: `Downloading ${attachments.length} attachment(s) as ZIP`,
      });
    } catch (error) {
      console.error('Error downloading all attachments:', error);
      toast({
        title: "Download Failed",
        description: "Failed to download attachments",
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

  const formatDate = (dateStr: string | null, internalDateStr: string | null) => {
    const dateToUse = dateStr || internalDateStr;
    if (!dateToUse) return 'No date';
    try {
      return format(new Date(dateToUse), 'MMM d, yyyy HH:mm');
    } catch {
      return dateToUse;
    }
  };

  if (!currentAccount) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email Body
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">No account selected</p>
        </CardContent>
      </Card>
    );
  }

  if (!selectedEmailContext) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email Body
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">Select an email to view</p>
        </CardContent>
      </Card>
    );
  }

  if (loading) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email Body
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex items-center justify-center">
          <RefreshCw className="h-8 w-8 animate-spin text-gray-400" />
        </CardContent>
      </Card>
    );
  }

  if (!email) {
    return (
      <Card className="h-full flex flex-col min-h-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email Body
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">Email not found</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col min-h-0">
      <CardHeader className="flex-shrink-0">
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-3">
            <Mail className="h-5 w-5" />
            Email Body
          </CardTitle>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={!email}
              title="Reply to this email"
            >
              <Reply className="h-4 w-4 mr-1" />
              Reply
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={!email}
              title="Forward this email"
            >
              <Forward className="h-4 w-4 mr-1" />
              Forward
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex-1 overflow-y-auto">
        <div className="mb-4">
          <h3 className="text-lg font-semibold mb-2">{email.subject || '(No subject)'}</h3>
          <p className="text-sm text-gray-600">
            From: {email.from_name || email.from_address || 'Unknown'}
          </p>
          {email.to_addresses && email.to_addresses.length > 0 && (
            <p className="text-sm text-gray-600">
              To: {email.to_addresses.join(', ')}
            </p>
          )}
          <p className="text-sm text-gray-600">
            Date: {formatDate(email.date, email.internal_date)}
          </p>
        </div>

        {/* View Mode Toggle and Show Images Button */}
        <div className="mb-4 border-t pt-4">
          <div className="flex gap-2 mb-4">
            {email.body_html && (
              <Button
                size="sm"
                variant={viewMode === 'html' ? 'default' : 'outline'}
                onClick={() => setViewMode('html')}
              >
                <Code className="h-4 w-4 mr-1" />
                HTML
              </Button>
            )}
            <Button
              size="sm"
              variant={viewMode === 'text' ? 'default' : 'outline'}
              onClick={() => setViewMode('text')}
            >
              <FileText className="h-4 w-4 mr-1" />
              Plain Text
            </Button>
            {viewMode === 'html' && email.body_html && !showImages && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => setShowImages(true)}
              >
                <Image className="h-4 w-4 mr-1" />
                Show Images
              </Button>
            )}
          </div>

          {/* Email Body Content */}
          {viewMode === 'html' && email.body_html ? (
            <div
              className="email-html-content"
              dangerouslySetInnerHTML={{
                __html: sanitizeEmailHtml(email.body_html, {
                  showImages,
                  messageId: email.message_id || undefined,
                  accountId: currentAccount?.id
                })
              }}
            />
          ) : (
            <div className="whitespace-pre-wrap">
              {email.body_text || 'No content'}
            </div>
          )}
        </div>

        {/* Attachments Section */}
        {loadingAttachments ? (
          <div className="mb-4 flex items-center gap-2 text-sm text-gray-600">
            <RefreshCw className="h-4 w-4 animate-spin" />
            Loading attachments...
          </div>
        ) : attachments.length > 0 ? (
          <div className="mb-4 border-t pt-4">
            <div className="flex items-center justify-between mb-2">
              <h4 className="text-sm font-semibold flex items-center gap-2">
                <Paperclip className="h-4 w-4" />
                Attachments ({attachments.length})
              </h4>
              <Button
                size="sm"
                variant="outline"
                onClick={downloadAllAttachments}
              >
                <Archive className="h-4 w-4 mr-2" />
                Download All as ZIP
              </Button>
            </div>
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
      </CardContent>
    </Card>
  );
};

export default EmailBody;
