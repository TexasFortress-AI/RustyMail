
import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { 
  Table, 
  TableBody, 
  TableCell, 
  TableHead, 
  TableHeader, 
  TableRow 
} from '@/components/ui/table';
import { 
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { useClients } from '@/dashboard/api/hooks';
import { formatDistanceToNow } from 'date-fns';
import { Loader2, Clock, User, Filter, ChevronsLeft, ChevronsRight } from 'lucide-react';

const ClientListPanel: React.FC = () => {
  const [page, setPage] = useState(1);
  const [filter, setFilter] = useState<string | undefined>(undefined);
  const { data, isLoading, isError } = useClients(page, 10, filter);

  // Status badge variants
  const getStatusBadge = (status: string) => {
    switch (status.toLowerCase()) {
      case 'active':
        return <Badge variant="outline" className="bg-green-500/10 text-green-600 border-green-200">Active</Badge>;
      case 'idle':
        return <Badge variant="outline" className="bg-amber-500/10 text-amber-600 border-amber-200">Idle</Badge>;
      case 'disconnecting':
        return <Badge variant="outline" className="bg-red-500/10 text-red-600 border-red-200">Disconnecting</Badge>;
      default:
        return <Badge variant="outline">{status}</Badge>;
    }
  };

  // Type badge variants
  const getTypeBadge = (type: string) => {
    switch (type.toLowerCase()) {
      case 'sse':
        return <Badge variant="secondary" className="bg-blue-500/10 text-blue-600 border-blue-200">SSE</Badge>;
      case 'api':
        return <Badge variant="secondary" className="bg-purple-500/10 text-purple-600 border-purple-200">API</Badge>;
      case 'console':
        return <Badge variant="secondary" className="bg-slate-500/10 text-slate-600 border-slate-200">Console</Badge>;
      default:
        return <Badge variant="secondary">{type}</Badge>;
    }
  };

  // Handle pagination
  const handlePageChange = (newPage: number) => {
    if (newPage > 0 && (!data || newPage <= data.pagination.totalPages)) {
      setPage(newPage);
    }
  };

  // Filter options
  const filterOptions = [
    { value: undefined, label: 'All Clients' },
    { value: 'active', label: 'Active' },
    { value: 'idle', label: 'Idle' },
    { value: 'disconnecting', label: 'Disconnecting' },
    { value: 'sse', label: 'SSE Clients' },
    { value: 'api', label: 'API Clients' },
    { value: 'console', label: 'Console Clients' },
  ];

  return (
    <Card className="shadow-sm transition-all duration-200 animate-fade-in glass-panel h-full flex flex-col" data-testid="client-list-panel">
      <CardHeader className="pb-2 flex-shrink-0">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg font-medium flex items-center">
            <span>Connected Clients</span>
            {data && (
              <Badge variant="outline" className="ml-2 bg-primary/10 border-primary/20">
                {data.pagination.total}
              </Badge>
            )}
          </CardTitle>
          
          <Select 
            defaultValue={filter} 
            onValueChange={(value) => setFilter(value === 'undefined' ? undefined : value)}
          >
            <SelectTrigger className="w-[160px] h-8 text-sm">
              <SelectValue placeholder="Filter clients">
                <div className="flex items-center">
                  <Filter className="h-3.5 w-3.5 mr-2" />
                  <span>Filter</span>
                </div>
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {filterOptions.map((option) => (
                <SelectItem 
                  key={option.label} 
                  value={option.value === undefined ? 'undefined' : option.value}
                  className="cursor-pointer"
                >
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </CardHeader>
      
      <CardContent className="pb-4 flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="h-64 flex items-center justify-center">
            <div className="animate-pulse flex flex-col items-center gap-3">
              <Loader2 className="h-8 w-8 animate-spin text-primary" />
              <p className="text-sm text-muted-foreground">Loading clients...</p>
            </div>
          </div>
        ) : isError ? (
          <div className="h-64 flex items-center justify-center">
            <div className="text-destructive flex flex-col items-center">
              <p className="text-sm">Failed to load client list</p>
              <button 
                className="mt-2 text-xs text-primary hover:underline"
                onClick={() => setPage(1)}
              >
                Try again
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="rounded-lg border overflow-hidden animate-slide-up" style={{ animationDelay: '0.2s' }}>
              <Table>
                <TableHeader className="bg-muted/30">
                  <TableRow>
                    <TableHead className="font-medium">Client ID</TableHead>
                    <TableHead className="font-medium">Type</TableHead>
                    <TableHead className="font-medium hidden md:table-cell">Connected</TableHead>
                    <TableHead className="font-medium text-right">Status</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data?.clients.map((client) => (
                    <TableRow key={client.id} className="bg-card/30 hover:bg-card/60 transition-colors">
                      <TableCell className="font-mono text-xs overflow-hidden text-ellipsis">
                        <div className="flex items-center">
                          <User className="h-3.5 w-3.5 text-muted-foreground mr-2" />
                          <span>{client.id}</span>
                        </div>
                      </TableCell>
                      <TableCell>
                        {getTypeBadge(client.type)}
                      </TableCell>
                      <TableCell className="hidden md:table-cell text-sm text-muted-foreground">
                        <div className="flex items-center">
                          <Clock className="h-3.5 w-3.5 mr-1.5" />
                          <span>{formatDistanceToNow(new Date(client.connectedAt), { addSuffix: true })}</span>
                        </div>
                      </TableCell>
                      <TableCell className="text-right">
                        {getStatusBadge(client.status)}
                      </TableCell>
                    </TableRow>
                  ))}
                  
                  {data?.clients.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={4} className="text-center text-muted-foreground py-8">
                        No clients found with the current filter
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </div>
            
            {data && data.pagination.totalPages > 1 && (
              <div className="flex items-center justify-between mt-4 animate-slide-up" style={{ animationDelay: '0.3s' }}>
                <div className="flex gap-1">
                  <Button
                    onClick={() => handlePageChange(1)}
                    disabled={page === 1}
                    size="sm"
                    variant="outline"
                    title="First page"
                  >
                    <ChevronsLeft className="h-4 w-4" />
                  </Button>
                  <Button
                    onClick={() => handlePageChange(page - 1)}
                    disabled={page === 1}
                    size="sm"
                    variant="outline"
                  >
                    Previous
                  </Button>
                </div>

                <span className="text-sm text-muted-foreground">
                  Page {page} of {data.pagination.totalPages}
                </span>

                <div className="flex gap-1">
                  <Button
                    onClick={() => handlePageChange(page + 1)}
                    disabled={page >= data.pagination.totalPages}
                    size="sm"
                    variant="outline"
                  >
                    Next
                  </Button>
                  <Button
                    onClick={() => handlePageChange(data.pagination.totalPages)}
                    disabled={page >= data.pagination.totalPages}
                    size="sm"
                    variant="outline"
                    title="Last page"
                  >
                    <ChevronsRight className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
};

export default ClientListPanel;
