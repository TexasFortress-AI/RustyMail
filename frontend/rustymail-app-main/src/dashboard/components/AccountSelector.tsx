import { useAccount } from '../../contexts/AccountContext';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../../components/ui/dropdown-menu';
import { Button } from '../../components/ui/button';
import { Badge } from '../../components/ui/badge';
import { ChevronDown, Mail, Check } from 'lucide-react';

export function AccountSelector() {
  const { currentAccount, accounts, switchAccount, loading } = useAccount();

  if (loading || !currentAccount) {
    return (
      <Button variant="outline" disabled>
        <Mail className="mr-2 h-4 w-4" />
        Loading...
      </Button>
    );
  }

  const activeAccounts = accounts.filter((a) => a.is_active);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" className="min-w-[200px] justify-between">
          <div className="flex items-center">
            <Mail className="mr-2 h-4 w-4" />
            <span className="truncate">{currentAccount.account_name}</span>
          </div>
          <ChevronDown className="ml-2 h-4 w-4 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[250px]">
        <DropdownMenuLabel>Email Accounts</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {activeAccounts.length === 0 ? (
          <DropdownMenuItem disabled>
            <span className="text-sm text-muted-foreground">No active accounts</span>
          </DropdownMenuItem>
        ) : (
          activeAccounts.map((account) => (
            <DropdownMenuItem
              key={account.id}
              onClick={() => switchAccount(account.id)}
              className="cursor-pointer"
            >
              <div className="flex items-center justify-between w-full">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium truncate">{account.account_name}</span>
                    {account.is_default && (
                      <Badge variant="secondary" className="text-xs">
                        Default
                      </Badge>
                    )}
                  </div>
                  <span className="text-xs text-muted-foreground truncate block">
                    {account.email_address}
                  </span>
                </div>
                {currentAccount.id === account.id && (
                  <Check className="ml-2 h-4 w-4 text-primary" />
                )}
              </div>
            </DropdownMenuItem>
          ))
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
