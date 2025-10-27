// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { useState, useEffect } from 'react';
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
import { Loader2, Send } from 'lucide-react';

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
}

export function SendMailDialog({
  open,
  onOpenChange,
  accountEmail,
  onSuccess,
  mode = 'compose',
  originalEmail,
}: SendMailDialogProps) {
  const { toast } = useToast();
  const [sending, setSending] = useState(false);
  const [sendingPhase, setSendingPhase] = useState<'idle' | 'preparing' | 'sending'>('idle');

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

  // Prefill form based on mode and originalEmail
  useEffect(() => {
    if (open && originalEmail) {
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
    } else if (open && mode === 'compose') {
      // Reset form for new compose
      setToInput('');
      setCcInput('');
      setBccInput('');
      setFormData({
        to: [],
        cc: [],
        bcc: [],
        subject: '',
        body: '',
        body_html: undefined,
      });
    }
  }, [open, mode, originalEmail]);

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
              disabled={sending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={sending}>
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
