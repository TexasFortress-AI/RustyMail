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

      // If no current account, set to default or first active account
      if (!currentAccount && accountsList.length > 0) {
        const defaultAccount = accountsList.find((a) => a.is_default && a.is_active);
        const firstActive = accountsList.find((a) => a.is_active);
        setCurrentAccount(defaultAccount || firstActive || accountsList[0]);
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

  // Initial load of accounts
  useEffect(() => {
    loadAccounts();
  }, []);

  // Restore last selected account from localStorage after accounts are loaded
  useEffect(() => {
    if (accounts.length === 0) return;

    const savedAccountId = localStorage.getItem('rustymail_current_account_id');
    if (savedAccountId && !currentAccount) {
      const account = accounts.find((a) => a.id === savedAccountId);
      if (account && account.is_active) {
        setCurrentAccount(account);
      }
    }
  }, [accounts]);

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
