// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { useState, useEffect, useRef } from 'react';
import { emailsApi, SendEmailRequest } from '../api/emails';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../components/ui/dialog';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Textarea } from '../../components/ui/textarea';
import { useToast } from '../../components/ui/use-toast';
import { Loader2, Send, Sparkles } from 'lucide-react';

interface SendMailDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  accountEmail?: string;
  onSuccess?: () => void;
  mode?: 'compose' | 'reply' | 'forward';
  originalEmail?: {
    subject: string | null;
    from_address: string | null;
    from_name: string | null;
    to_addresses: string[];
    body_text: string | null;
  };
  // Context needed for AI draft functionality
  emailContext?: {
    uid: number;
    folder: string;
  };
}

export function SendMailDialog({
  open,
  onOpenChange,
  accountEmail,
  onSuccess,
  mode = 'compose',
  originalEmail,
  emailContext,
}: SendMailDialogProps) {
  const { toast } = useToast();
  const [sending, setSending] = useState(false);
  const [sendingPhase, setSendingPhase] = useState<'idle' | 'preparing' | 'sending'>('idle');
  const [drafting, setDrafting] = useState(false);
  const [aiInstructions, setAiInstructions] = useState('');
  const [loadingInstructions, setLoadingInstructions] = useState(false);

  const [formData, setFormData] = useState<SendEmailRequest>({
    to: [],
    cc: [],
    bcc: [],
    subject: '',
    body: '',
    body_html: undefined,
  });

  const [toInput, setToInput] = useState('');
  const [ccInput, setCcInput] = useState('');
  const [bccInput, setBccInput] = useState('');

  // Debug logging for dialog open state
  useEffect(() => {
    console.log('[SendMailDialog] open prop changed:', open, 'mode:', mode);
    if (open) {
      console.trace('[SendMailDialog] Dialog opened - stack trace:');
    }
  }, [open]);

  // Track if we've already prefilled the form for this dialog session
  const hasPrefilled = useRef(false);

  // Prefill form based on mode and originalEmail (only once per dialog open)
  useEffect(() => {
    if (open && !hasPrefilled.current) {
      hasPrefilled.current = true;

      if (originalEmail) {
        if (mode === 'reply') {
          // Reply: Set TO to original sender
          setToInput(originalEmail.from_address || '');
          // Prefix subject with Re: if not already present
          const subject = originalEmail.subject || '';
          const newSubject = subject.toLowerCase().startsWith('re:') ? subject : `Re: ${subject}`;
          setFormData(prev => ({
            ...prev,
            subject: newSubject,
            body: `\n\n-------- Original Message --------\nFrom: ${originalEmail.from_name || originalEmail.from_address || 'Unknown'}\nSubject: ${originalEmail.subject || '(No subject)'}\n\n${originalEmail.body_text || ''}`
          }));
        } else if (mode === 'forward') {
          // Forward: Clear TO, prefix subject with Fwd:
          setToInput('');
          const subject = originalEmail.subject || '';
          const newSubject = subject.toLowerCase().startsWith('fwd:') || subject.toLowerCase().startsWith('fw:')
            ? subject
            : `Fwd: ${subject}`;
          setFormData(prev => ({
            ...prev,
            subject: newSubject,
            body: `\n\n-------- Forwarded Message --------\nFrom: ${originalEmail.from_name || originalEmail.from_address || 'Unknown'}\nTo: ${originalEmail.to_addresses.join(', ')}\nSubject: ${originalEmail.subject || '(No subject)'}\n\n${originalEmail.body_text || ''}`
          }));
        }
      } else if (mode === 'compose') {
        // Reset form for new compose
        setToInput('');
        setCcInput('');
        setBccInput('');
        setAiInstructions('');
        setFormData({
          to: [],
          cc: [],
          bcc: [],
          subject: '',
          body: '',
          body_html: undefined,
        });
      }
    } else if (!open) {
      // Reset the prefill flag when dialog closes
      hasPrefilled.current = false;
    }
  }, [open, mode]);

  // Track if we've already fetched instructions for this dialog session
  const hasFetchedInstructions = useRef(false);

  // Fetch AI-generated instructions when dialog opens in reply mode (only once per open)
  useEffect(() => {
    if (open && mode === 'reply' && originalEmail && emailContext && !hasFetchedInstructions.current) {
      hasFetchedInstructions.current = true;
      const fetchInstructions = async () => {
        setLoadingInstructions(true);
        setAiInstructions('');
        try {
          const result = await emailsApi.suggestReplyInstructions({
            subject: originalEmail.subject || '',
            from: originalEmail.from_name || originalEmail.from_address || 'Unknown',
            body_preview: originalEmail.body_text || '',
          });
          if (result.success && result.instruction) {
            // Strip any provider/model prefix like "[Provider: xxx, Model: xxx]"
            let instruction = result.instruction;
            instruction = instruction.replace(/^\[Provider:.*?\]\s*/i, '');
            setAiInstructions(instruction);
          } else {
            setAiInstructions('Write a professional reply');
          }
        } catch {
          setAiInstructions('Write a professional reply');
        } finally {
          setLoadingInstructions(false);
        }
      };
      fetchInstructions();
    } else if (!open) {
      setAiInstructions('');
      hasFetchedInstructions.current = false; // Reset for next open
    }
  }, [open, mode]);

  // Handle AI draft for reply
  const handleAiDraft = async () => {
    if (!emailContext || !accountEmail) {
      toast({
        title: 'Cannot Draft',
        description: 'Email context is required for AI drafting',
        variant: 'destructive',
      });
      return;
    }

    setDrafting(true);
    try {
      toast({
        title: 'Drafting Reply',
        description: 'AI is generating a reply...',
      });

      console.log('[SendMailDialog] Drafting reply with:', {
        email_uid: emailContext.uid,
        folder: emailContext.folder,
        account_id: accountEmail,
        instructions: aiInstructions || undefined,
      });

      const response = await emailsApi.draftReply({
        email_uid: emailContext.uid,
        folder: emailContext.folder,
        account_id: accountEmail,
        instructions: aiInstructions || undefined,
      });

      console.log('[SendMailDialog] Draft response:', response);

      if (response.success && response.data?.draft) {
        // Update the body with the AI draft, keeping the original message quote
        const originalQuote = formData.body;
        const newBody = response.data.draft + '\n\n' + originalQuote;
        console.log('[SendMailDialog] Draft text:', response.data.draft);
        console.log('[SendMailDialog] Original quote:', originalQuote);
        console.log('[SendMailDialog] New body will be:', newBody);

        setFormData(prev => {
          console.log('[SendMailDialog] Previous formData:', prev);
          const updated = {
            ...prev,
            body: newBody,
          };
          console.log('[SendMailDialog] Updated formData:', updated);
          return updated;
        });

        toast({
          title: 'Draft Ready',
          description: 'AI has drafted a reply for you',
        });
      } else {
        // Log full response for debugging
        console.error('[SendMailDialog] Draft failed, full response:', JSON.stringify(response, null, 2));
        throw new Error(response.error || `Failed to generate draft: ${JSON.stringify(response)}`);
      }
    } catch (error) {
      console.error('AI draft failed:', error);
      toast({
        title: 'Draft Failed',
        description: error instanceof Error ? error.message : 'Failed to generate AI draft',
        variant: 'destructive',
      });
    } finally {
      setDrafting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // Validate account email is present
    if (!accountEmail) {
      toast({
        title: 'No Account Selected',
        description: 'Please select an email account before sending',
        variant: 'destructive',
      });
      return;
    }

    // Parse comma-separated email lists
    const toEmails = toInput
      .split(',')
      .map((email) => email.trim())
      .filter((email) => email);
    const ccEmails = ccInput
      .split(',')
      .map((email) => email.trim())
      .filter((email) => email);
    const bccEmails = bccInput
      .split(',')
      .map((email) => email.trim())
      .filter((email) => email);

    if (toEmails.length === 0) {
      toast({
        title: 'Required Fields Missing',
        description: 'Please enter at least one recipient',
        variant: 'destructive',
      });
      return;
    }

    if (!formData.subject || !formData.body) {
      toast({
        title: 'Required Fields Missing',
        description: 'Please fill in subject and message',
        variant: 'destructive',
      });
      return;
    }

    try {
      setSending(true);
      setSendingPhase('preparing');

      // Show informational toast about potential delays
      toast({
        title: 'Preparing Email',
        description: 'Saving to Outbox and sending (this may take up to 40 seconds with some email servers)...',
      });

      const request: SendEmailRequest = {
        to: toEmails,
        cc: ccEmails.length > 0 ? ccEmails : undefined,
        bcc: bccEmails.length > 0 ? bccEmails : undefined,
        subject: formData.subject,
        body: formData.body,
        body_html: formData.body_html,
      };

      // Transition to sending phase after a brief moment
      setTimeout(() => setSendingPhase('sending'), 1000);

      await emailsApi.sendEmail(request, accountEmail);

      toast({
        title: 'Success',
        description: 'Email sent successfully',
      });

      // Reset form
      setFormData({
        to: [],
        cc: [],
        bcc: [],
        subject: '',
        body: '',
        body_html: undefined,
      });
      setToInput('');
      setCcInput('');
      setBccInput('');

      if (onSuccess) {
        onSuccess();
      }
      onOpenChange(false);
    } catch (error: any) {
      console.error('Failed to send email:', error);
      toast({
        title: 'Error',
        description: error.message || 'Failed to send email',
        variant: 'destructive',
      });
    } finally {
      setSending(false);
      setSendingPhase('idle');
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Compose Email</DialogTitle>
          <DialogDescription>
            {accountEmail ? (
              <>
                From: <span className="font-semibold">{accountEmail}</span>
              </>
            ) : (
              'Send a new email message'
            )}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="to">To *</Label>
              <Input
                id="to"
                type="email"
                value={toInput}
                onChange={(e) => setToInput(e.target.value)}
                placeholder="recipient@example.com, another@example.com"
                required
              />
              <p className="text-xs text-muted-foreground">
                Separate multiple email addresses with commas
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="cc">CC</Label>
              <Input
                id="cc"
                type="email"
                value={ccInput}
                onChange={(e) => setCcInput(e.target.value)}
                placeholder="cc@example.com"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="bcc">BCC</Label>
              <Input
                id="bcc"
                type="email"
                value={bccInput}
                onChange={(e) => setBccInput(e.target.value)}
                placeholder="bcc@example.com"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="subject">Subject *</Label>
              <Input
                id="subject"
                value={formData.subject}
                onChange={(e) =>
                  setFormData({ ...formData, subject: e.target.value })
                }
                placeholder="Email subject"
                required
              />
            </div>

            {mode === 'reply' && emailContext && (
              <div className="space-y-2">
                <Label htmlFor="aiInstructions" className="flex items-center gap-2">
                  <Sparkles className="h-4 w-4" />
                  AI Instructions
                  {loadingInstructions && (
                    <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
                  )}
                </Label>
                <Input
                  id="aiInstructions"
                  value={aiInstructions}
                  onChange={(e) => setAiInstructions(e.target.value)}
                  placeholder={loadingInstructions ? 'Generating suggestion...' : 'e.g., Politely decline, Accept and ask about timeline...'}
                  disabled={loadingInstructions}
                />
                <p className="text-xs text-muted-foreground">
                  Edit these instructions to customize how the AI drafts your reply
                </p>
              </div>
            )}

            <div className="space-y-2">
              <Label htmlFor="body">Message *</Label>
              <Textarea
                id="body"
                value={formData.body}
                onChange={(e) =>
                  setFormData({ ...formData, body: e.target.value })
                }
                placeholder="Enter your message"
                rows={10}
                required
              />
            </div>
          </div>

          <DialogFooter className="mt-6">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={sending || drafting}
            >
              Cancel
            </Button>
            {mode === 'reply' && emailContext && (
              <Button
                type="button"
                variant="secondary"
                onClick={handleAiDraft}
                disabled={sending || drafting}
              >
                {drafting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Drafting...
                  </>
                ) : (
                  <>
                    <Sparkles className="mr-2 h-4 w-4" />
                    Draft with AI
                  </>
                )}
              </Button>
            )}
            <Button type="submit" disabled={sending || drafting}>
              {sending ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {sendingPhase === 'preparing' ? 'Saving to Outbox...' : 'Sending via SMTP...'}
                </>
              ) : (
                <>
                  <Send className="mr-2 h-4 w-4" />
                  Send Email
                </>
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
