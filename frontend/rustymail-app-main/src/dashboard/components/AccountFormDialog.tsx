import { useState, useEffect } from 'react';
import { Account, AccountFormData, AutoConfigResult } from '../../types';
import { accountsApi } from '../api/accounts';
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
import { Switch } from '../../components/ui/switch';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import { useToast } from '../../components/ui/use-toast';
import { Loader2, Wand2, CheckCircle2, XCircle } from 'lucide-react';
import { Badge } from '../../components/ui/badge';
import { ConnectionStatusIndicator } from './ConnectionStatusIndicator';

interface AccountFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  account?: Account | null;
  onSuccess: () => void;
}

export function AccountFormDialog({
  open,
  onOpenChange,
  account,
  onSuccess,
}: AccountFormDialogProps) {
  const { toast } = useToast();
  const [loading, setLoading] = useState(false);
  const [autoConfiguring, setAutoConfiguring] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validationResult, setValidationResult] = useState<{
    success: boolean;
    message?: string;
  } | null>(null);
  const [autoDiscoveryCompleted, setAutoDiscoveryCompleted] = useState(false);

  const [formData, setFormData] = useState<AccountFormData>({
    account_name: '',
    email_address: '',
    provider_type: undefined,
    imap_host: '',
    imap_port: 993,
    imap_user: '',
    imap_pass: '',
    imap_use_tls: true,
    smtp_host: '',
    smtp_port: 587,
    smtp_user: '',
    smtp_pass: '',
    smtp_use_tls: true,
    smtp_use_starttls: true,
    is_default: false,
    validate_connection: true,
  });

  const [autoConfigResult, setAutoConfigResult] = useState<AutoConfigResult | null>(null);

  useEffect(() => {
    if (account) {
      setFormData({
        account_name: account.account_name,
        email_address: account.email_address,
        provider_type: account.provider_type,
        imap_host: account.imap_host,
        imap_port: account.imap_port,
        imap_user: account.imap_user,
        imap_pass: '', // Don't pre-fill password for security
        imap_use_tls: account.imap_use_tls,
        smtp_host: account.smtp_host,
        smtp_port: account.smtp_port,
        smtp_user: account.smtp_user,
        smtp_pass: '',
        smtp_use_tls: account.smtp_use_tls,
        smtp_use_starttls: account.smtp_use_starttls,
        is_default: account.is_default,
        validate_connection: false,
      });
    } else {
      // Reset form for new account
      setFormData({
        account_name: '',
        email_address: '',
        provider_type: undefined,
        imap_host: '',
        imap_port: 993,
        imap_user: '',
        imap_pass: '',
        imap_use_tls: true,
        smtp_host: '',
        smtp_port: 587,
        smtp_user: '',
        smtp_pass: '',
        smtp_use_tls: true,
        smtp_use_starttls: true,
        is_default: false,
        validate_connection: true,
      });
      setAutoConfigResult(null);
      setValidationResult(null);
    }
  }, [account, open]);

  // Auto-save after successful autodiscovery
  useEffect(() => {
    if (autoDiscoveryCompleted && formData.imap_host) {
      setAutoDiscoveryCompleted(false); // Reset flag
      saveAccount(); // Automatically save the account
    }
  }, [autoDiscoveryCompleted, formData.imap_host]);

  const handleAutoConfig = async () => {
    if (!formData.email_address) {
      toast({
        title: 'Email Required',
        description: 'Please enter an email address first',
        variant: 'destructive',
      });
      return;
    }

    try {
      setAutoConfiguring(true);
      const result = await accountsApi.autoConfig(formData.email_address, formData.imap_pass);
      setAutoConfigResult(result);

      // Check if autodiscovery succeeded (new format)
      const hasConfig = result.imap_host && (result.smtp_host || result.smtp_host === '');

      if (hasConfig) {
        setFormData((prev) => ({
          ...prev,
          provider_type: result.provider_name || result.provider_type || 'Auto-discovered',
          imap_host: result.imap_host!,
          imap_port: result.imap_port || 993,
          imap_use_tls: result.imap_use_tls !== undefined ? result.imap_use_tls : true,
          imap_user: prev.email_address,
          smtp_host: result.smtp_host || '',
          smtp_port: result.smtp_port || 587,
          smtp_use_tls: result.smtp_use_tls !== undefined ? result.smtp_use_tls : false,
          smtp_use_starttls: result.smtp_use_starttls !== undefined ? result.smtp_use_starttls : true,
          smtp_user: prev.email_address,
          account_name: prev.account_name || result.display_name || result.provider_name || prev.email_address,
        }));

        // Set flag to trigger automatic save after autodiscovery
        setAutoDiscoveryCompleted(true);

        toast({
          title: 'Auto-Configuration Successful',
          description: `Found settings for ${result.display_name || result.provider_name || 'your email provider'}`,
        });
      } else {
        // Fallback to old format check
        if (result.provider_found) {
          toast({
            title: 'Provider Found',
            description: `Settings for ${result.display_name || result.provider_type} found`,
          });
        } else {
          toast({
            title: 'Provider Not Found',
            description: 'Could not auto-discover email settings. Please configure manually.',
          });
        }
      }
    } catch (error: any) {
      console.error('Auto-config failed:', error);
      toast({
        title: 'Auto-Configuration Failed',
        description: error.message || 'Could not auto-discover email settings. Please configure manually.',
        variant: 'destructive',
      });
    } finally {
      setAutoConfiguring(false);
    }
  };

  const handleValidate = async () => {
    if (!account?.id) return;

    try {
      setValidating(true);
      const result = await accountsApi.validateConnection(account.id);
      setValidationResult(result);

      if (result.success) {
        toast({
          title: 'Connection Successful',
          description: 'Account credentials are valid',
        });
      } else {
        toast({
          title: 'Connection Failed',
          description: result.message || 'Please check your credentials',
          variant: 'destructive',
        });
      }
    } catch (error) {
      console.error('Validation failed:', error);
      setValidationResult({
        success: false,
        message: 'Validation failed',
      });
    } finally {
      setValidating(false);
    }
  };

  // Extract save logic into a separate function
  const saveAccount = async () => {
    try {
      setLoading(true);

      if (account) {
        // Update existing account
        await accountsApi.updateAccount(account.id, formData);
        toast({
          title: 'Success',
          description: 'Account updated successfully',
        });
      } else {
        // Create new account
        await accountsApi.createAccount(formData);
        toast({
          title: 'Success',
          description: 'Account created successfully',
        });
      }

      onSuccess();
      onOpenChange(false);
    } catch (error: any) {
      console.error('Failed to save account:', error);
      toast({
        title: 'Error',
        description: error.message || 'Failed to save account',
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // Check basic required fields
    // Password is only required when creating a new account, not when editing
    if (!formData.account_name || !formData.email_address || (!formData.imap_pass && !account)) {
      toast({
        title: 'Required Fields Missing',
        description: 'Please fill in all required fields',
        variant: 'destructive',
      });
      return;
    }

    // If IMAP host is not set, try autodiscovery first
    if (!formData.imap_host) {
      toast({
        title: 'Auto-discovering settings...',
        description: 'Please wait while we detect your email server configuration',
      });

      try {
        await handleAutoConfig();
        // After autodiscovery completes, the autoDiscoveryCompleted flag will be set
        // which triggers a useEffect to continue with saving
        return;
      } catch (error) {
        toast({
          title: 'Auto-discovery Failed',
          description: 'Please manually configure IMAP/SMTP settings in the IMAP and SMTP tabs',
          variant: 'destructive',
        });
        return;
      }
    }

    // Proceed with saving the account
    await saveAccount();
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{account ? 'Edit Account' : 'Add Email Account'}</DialogTitle>
          <DialogDescription>
            {account
              ? 'Update your email account settings'
              : 'Configure your email account to start receiving messages'}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <Tabs defaultValue="basic" className="w-full">
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="basic">Basic</TabsTrigger>
              <TabsTrigger value="imap">IMAP</TabsTrigger>
              <TabsTrigger value="smtp">SMTP</TabsTrigger>
            </TabsList>

            <TabsContent value="basic" className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="email_address">Email Address *</Label>
                <div className="flex space-x-2">
                  <Input
                    id="email_address"
                    type="email"
                    value={formData.email_address}
                    onChange={(e) =>
                      setFormData({ ...formData, email_address: e.target.value })
                    }
                    placeholder="you@example.com"
                    required
                    disabled={!!account}
                  />
                  {!account && (
                    <Button
                      type="button"
                      variant="outline"
                      onClick={handleAutoConfig}
                      disabled={autoConfiguring || !formData.email_address}
                    >
                      {autoConfiguring ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Wand2 className="h-4 w-4" />
                      )}
                    </Button>
                  )}
                </div>
                {autoConfigResult && (
                  <div className="text-sm">
                    {autoConfigResult.provider_found ? (
                      <Badge variant="default" className="gap-1">
                        <CheckCircle2 className="h-3 w-3" />
                        {autoConfigResult.display_name}
                      </Badge>
                    ) : (
                      <Badge variant="secondary" className="gap-1">
                        <XCircle className="h-3 w-3" />
                        Manual configuration required
                      </Badge>
                    )}
                  </div>
                )}
              </div>

              <div className="space-y-2">
                <Label htmlFor="account_name">Account Name *</Label>
                <Input
                  id="account_name"
                  value={formData.account_name}
                  onChange={(e) =>
                    setFormData({ ...formData, account_name: e.target.value })
                  }
                  placeholder="Personal Gmail"
                  required
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="password">Password *</Label>
                <Input
                  id="password"
                  type="password"
                  value={formData.imap_pass}
                  onChange={(e) =>
                    setFormData({ ...formData, imap_pass: e.target.value, smtp_pass: e.target.value })
                  }
                  placeholder="Enter your email password"
                  required={!account}
                />
                <p className="text-xs text-muted-foreground">
                  Used for both IMAP and SMTP authentication
                </p>
              </div>

              <div className="flex items-center space-x-2">
                <Switch
                  id="is_default"
                  checked={formData.is_default}
                  onCheckedChange={(checked) =>
                    setFormData({ ...formData, is_default: checked })
                  }
                />
                <Label htmlFor="is_default">Set as default account</Label>
              </div>

              {account?.connection_status && (
                <div className="space-y-2">
                  <Label>Connection Status</Label>
                  <div className="flex gap-2">
                    <ConnectionStatusIndicator
                      label="IMAP"
                      attempt={account.connection_status.imap}
                    />
                    {account.smtp_host && (
                      <ConnectionStatusIndicator
                        label="SMTP"
                        attempt={account.connection_status.smtp}
                      />
                    )}
                  </div>
                </div>
              )}

              {account && (
                <div className="space-y-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={handleValidate}
                    disabled={validating}
                    className="w-full"
                  >
                    {validating ? (
                      <>
                        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        Testing Connection...
                      </>
                    ) : (
                      'Test Connection'
                    )}
                  </Button>
                  {validationResult && (
                    <div className="text-sm">
                      {validationResult.success ? (
                        <Badge variant="default" className="gap-1">
                          <CheckCircle2 className="h-3 w-3" />
                          Connection successful
                        </Badge>
                      ) : (
                        <Badge variant="destructive" className="gap-1">
                          <XCircle className="h-3 w-3" />
                          {validationResult.message}
                        </Badge>
                      )}
                    </div>
                  )}
                </div>
              )}
            </TabsContent>

            <TabsContent value="imap" className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="imap_host">IMAP Server *</Label>
                  <Input
                    id="imap_host"
                    value={formData.imap_host}
                    onChange={(e) =>
                      setFormData({ ...formData, imap_host: e.target.value })
                    }
                    placeholder="imap.gmail.com"
                    required
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="imap_port">Port *</Label>
                  <Input
                    id="imap_port"
                    type="number"
                    value={formData.imap_port}
                    onChange={(e) =>
                      setFormData({ ...formData, imap_port: parseInt(e.target.value) })
                    }
                    required
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="imap_user">Username *</Label>
                <Input
                  id="imap_user"
                  value={formData.imap_user}
                  onChange={(e) =>
                    setFormData({ ...formData, imap_user: e.target.value })
                  }
                  placeholder="Usually your email address"
                  required
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="imap_pass">Password *</Label>
                <Input
                  id="imap_pass"
                  type="password"
                  value={formData.imap_pass}
                  onChange={(e) =>
                    setFormData({ ...formData, imap_pass: e.target.value })
                  }
                  placeholder={account ? 'Leave blank to keep current' : ''}
                  required={!account}
                />
              </div>

              <div className="flex items-center space-x-2">
                <Switch
                  id="imap_use_tls"
                  checked={formData.imap_use_tls}
                  onCheckedChange={(checked) =>
                    setFormData({ ...formData, imap_use_tls: checked })
                  }
                />
                <Label htmlFor="imap_use_tls">Use TLS/SSL</Label>
              </div>
            </TabsContent>

            <TabsContent value="smtp" className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="smtp_host">SMTP Server</Label>
                  <Input
                    id="smtp_host"
                    value={formData.smtp_host}
                    onChange={(e) =>
                      setFormData({ ...formData, smtp_host: e.target.value })
                    }
                    placeholder="smtp.gmail.com"
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="smtp_port">Port</Label>
                  <Input
                    id="smtp_port"
                    type="number"
                    value={formData.smtp_port}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        smtp_port: parseInt(e.target.value) || undefined,
                      })
                    }
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="smtp_user">Username</Label>
                <Input
                  id="smtp_user"
                  value={formData.smtp_user}
                  onChange={(e) =>
                    setFormData({ ...formData, smtp_user: e.target.value })
                  }
                  placeholder="Usually your email address"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="smtp_pass">Password</Label>
                <Input
                  id="smtp_pass"
                  type="password"
                  value={formData.smtp_pass}
                  onChange={(e) =>
                    setFormData({ ...formData, smtp_pass: e.target.value })
                  }
                  placeholder={account ? 'Leave blank to keep current' : ''}
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center space-x-2">
                  <Switch
                    id="smtp_use_tls"
                    checked={formData.smtp_use_tls}
                    onCheckedChange={(checked) =>
                      setFormData({ ...formData, smtp_use_tls: checked })
                    }
                  />
                  <Label htmlFor="smtp_use_tls">Use TLS/SSL</Label>
                </div>

                <div className="flex items-center space-x-2">
                  <Switch
                    id="smtp_use_starttls"
                    checked={formData.smtp_use_starttls}
                    onCheckedChange={(checked) =>
                      setFormData({ ...formData, smtp_use_starttls: checked })
                    }
                  />
                  <Label htmlFor="smtp_use_starttls">Use STARTTLS</Label>
                </div>
              </div>
            </TabsContent>
          </Tabs>

          <DialogFooter className="mt-6">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={loading}>
              {loading ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Saving...
                </>
              ) : (
                'Save Account'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
