// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { useState } from 'react';
import { Account } from '../../types';
import { accountsApi } from '../api/accounts';
import { Card, CardContent, CardHeader, CardTitle } from '../../components/ui/card';
import { Button } from '../../components/ui/button';
import { Badge } from '../../components/ui/badge';
import { Skeleton } from '../../components/ui/skeleton';
import { useToast } from '../../components/ui/use-toast';
import { ConnectionStatusIndicator } from './ConnectionStatusIndicator';
import { useAccount } from '../../contexts/AccountContext';
import {
  Mail,
  Plus,
  Settings,
  Check,
  Trash2,
  AlertCircle,
  Shield,
} from 'lucide-react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '../../components/ui/alert-dialog';

interface AccountListPanelProps {
  onAddAccount: () => void;
  onEditAccount: (account: Account) => void;
  onAccountSwitch?: (account: Account) => void;
}

export function AccountListPanel({
  onAddAccount,
  onEditAccount,
  onAccountSwitch,
}: AccountListPanelProps) {
  const { accounts, loading, refreshAccounts } = useAccount();
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [accountToDelete, setAccountToDelete] = useState<Account | null>(null);
  const { toast } = useToast();

  const handleSetDefault = async (account: Account) => {
    if (account.is_default) return;

    try {
      await accountsApi.setDefaultAccount(account.id);
      toast({
        title: 'Success',
        description: `${account.account_name} is now the default account`,
      });
      await refreshAccounts();
    } catch (error) {
      console.error('Failed to set default account:', error);
      toast({
        title: 'Error',
        description: 'Failed to set default account',
        variant: 'destructive',
      });
    }
  };

  const handleDelete = async () => {
    if (!accountToDelete) return;

    try {
      await accountsApi.deleteAccount(accountToDelete.id);
      toast({
        title: 'Success',
        description: `Account ${accountToDelete.account_name} deleted`,
      });
      setDeleteDialogOpen(false);
      setAccountToDelete(null);
      await refreshAccounts();
    } catch (error) {
      console.error('Failed to delete account:', error);
      toast({
        title: 'Error',
        description: 'Failed to delete account',
        variant: 'destructive',
      });
    }
  };

  const openDeleteDialog = (account: Account) => {
    setAccountToDelete(account);
    setDeleteDialogOpen(true);
  };

  const handleAccountClick = (account: Account) => {
    if (onAccountSwitch && account.is_active) {
      onAccountSwitch(account);
    }
  };

  if (loading) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Email Accounts</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {[1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-20 w-full" />
          ))}
        </CardContent>
      </Card>
    );
  }

  return (
    <>
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
          <CardTitle>Email Accounts</CardTitle>
          <Button onClick={onAddAccount} size="sm">
            <Plus className="mr-2 h-4 w-4" />
            Add Account
          </Button>
        </CardHeader>
        <CardContent>
          {accounts.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-center">
              <Mail className="h-12 w-12 text-muted-foreground mb-4" />
              <p className="text-sm text-muted-foreground mb-4">
                No email accounts configured yet
              </p>
              <Button onClick={onAddAccount}>
                <Plus className="mr-2 h-4 w-4" />
                Add Your First Account
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {accounts.map((account) => (
                <div
                  key={account.id}
                  className={`
                    flex items-center justify-between p-4 border rounded-lg
                    ${account.is_active ? 'cursor-pointer hover:bg-accent' : 'opacity-60'}
                    ${account.is_default ? 'border-primary' : ''}
                  `}
                  onClick={() => handleAccountClick(account)}
                >
                  <div className="flex items-center space-x-3 flex-1">
                    <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center">
                      <Mail className="h-5 w-5 text-primary" />
                    </div>
                    <div className="flex-1">
                      <div className="flex items-center space-x-2">
                        <h3 className="font-semibold">{account.account_name}</h3>
                        {account.is_default && (
                          <Badge variant="default" className="text-xs">
                            <Check className="mr-1 h-3 w-3" />
                            Default
                          </Badge>
                        )}
                        {!account.is_active && (
                          <Badge variant="secondary" className="text-xs">
                            <AlertCircle className="mr-1 h-3 w-3" />
                            Inactive
                          </Badge>
                        )}
                        {account.oauth_provider && (
                          <Badge variant="outline" className="text-xs gap-1" style={{ borderColor: '#0078D4', color: '#0078D4' }}>
                            <Shield className="h-3 w-3" />
                            OAuth
                          </Badge>
                        )}
                        {account.provider_type && (
                          <Badge variant="outline" className="text-xs">
                            {account.provider_type}
                          </Badge>
                        )}
                      </div>
                      <p className="text-sm text-muted-foreground">
                        {account.email_address}
                      </p>
                      {account.connection_status && (
                        <div className="flex items-center gap-2 mt-2" onClick={(e) => e.stopPropagation()}>
                          <ConnectionStatusIndicator
                            label="IMAP"
                            attempt={account.connection_status.imap}
                            compact
                          />
                          {account.smtp_host && (
                            <ConnectionStatusIndicator
                              label="SMTP"
                              attempt={account.connection_status.smtp}
                              compact
                            />
                          )}
                        </div>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center space-x-2">
                    {!account.is_default && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSetDefault(account);
                        }}
                      >
                        Set Default
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={(e) => {
                        e.stopPropagation();
                        onEditAccount(account);
                      }}
                    >
                      <Settings className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={(e) => {
                        e.stopPropagation();
                        openDeleteDialog(account);
                      }}
                    >
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Account</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete {accountToDelete?.account_name}?
              This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDelete} className="bg-destructive">
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
