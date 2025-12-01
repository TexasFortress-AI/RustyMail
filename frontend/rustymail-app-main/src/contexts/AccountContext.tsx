// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { Account } from '../types';
import { accountsApi } from '../dashboard/api/accounts';

interface AccountContextType {
  currentAccount: Account | null;
  accounts: Account[];
  loading: boolean;
  switchAccount: (accountId: string) => Promise<void>;
  refreshAccounts: () => Promise<void>;
  setCurrentAccount: (account: Account | null) => void;
}

const AccountContext = createContext<AccountContextType | undefined>(undefined);

export function AccountProvider({ children }: { children: ReactNode }) {
  const [currentAccount, setCurrentAccount] = useState<Account | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [loading, setLoading] = useState(true);

  const loadAccounts = async () => {
    try {
      setLoading(true);
      const accountsList = await accountsApi.listAccounts();
      setAccounts(accountsList);

      // If current account exists, update it from refreshed list (to get name changes, etc.)
      if (currentAccount) {
        const updatedCurrentAccount = accountsList.find((a) => a.id === currentAccount.id);
        if (updatedCurrentAccount) {
          setCurrentAccount(updatedCurrentAccount);
        } else {
          // Current account was deleted, switch to default or first active
          const defaultAccount = accountsList.find((a) => a.is_default && a.is_active);
          const firstActive = accountsList.find((a) => a.is_active);
          setCurrentAccount(defaultAccount || firstActive || accountsList[0] || null);
        }
      } else if (accountsList.length > 0) {
        // No current account, try localStorage first, then fall back to default
        const savedAccountId = localStorage.getItem('rustymail_current_account_id');
        let accountToSet: Account | null = null;

        // Try to restore from localStorage
        if (savedAccountId) {
          accountToSet = accountsList.find((a) => a.id === savedAccountId && a.is_active) || null;
        }

        // Fall back to default or first active account
        if (!accountToSet) {
          const defaultAccount = accountsList.find((a) => a.is_default && a.is_active);
          const firstActive = accountsList.find((a) => a.is_active);
          accountToSet = defaultAccount || firstActive || accountsList[0];
        }

        setCurrentAccount(accountToSet);
      }
    } catch (error) {
      console.error('Failed to load accounts:', error);
    } finally {
      setLoading(false);
    }
  };

  const switchAccount = async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (account && account.is_active) {
      setCurrentAccount(account);

      // Store in localStorage for persistence
      localStorage.setItem('rustymail_current_account_id', accountId);
    }
  };

  const refreshAccounts = async () => {
    await loadAccounts();
  };

  // Initial load of accounts (localStorage restoration handled in loadAccounts)
  useEffect(() => {
    loadAccounts();
  }, []);

  return (
    <AccountContext.Provider
      value={{
        currentAccount,
        accounts,
        loading,
        switchAccount,
        refreshAccounts,
        setCurrentAccount,
      }}
    >
      {children}
    </AccountContext.Provider>
  );
}

export function useAccount() {
  const context = useContext(AccountContext);
  if (context === undefined) {
    throw new Error('useAccount must be used within an AccountProvider');
  }
  return context;
}
