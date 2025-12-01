// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Loader2, Play, X, RefreshCw, Clock, CheckCircle, XCircle, AlertCircle } from 'lucide-react';
import { useJobs, useCancelJob, useStartProcessEmailsJob } from '@/dashboard/api/hooks';
import { useAccount } from '@/contexts/AccountContext';

type Job = {
  job_id: string;
  instruction: string | null;
  status: string;
  result_data: string | null;
  error_message: string | null;
  started_at: string;
  updated_at: string;
  completed_at: string | null;
  resumable: boolean;
  retry_count: number;
  max_retries: number;
};

const JobStatusBadge: React.FC<{ status: string }> = ({ status }) => {
  switch (status) {
    case 'running':
      return (
        <Badge variant="default" className="bg-blue-500">
          <Loader2 className="w-3 h-3 mr-1 animate-spin" />
          Running
        </Badge>
      );
    case 'completed':
      return (
        <Badge variant="default" className="bg-green-500">
          <CheckCircle className="w-3 h-3 mr-1" />
          Completed
        </Badge>
      );
    case 'failed':
      return (
        <Badge variant="destructive">
          <XCircle className="w-3 h-3 mr-1" />
          Failed
        </Badge>
      );
    case 'cancelled':
      return (
        <Badge variant="secondary">
          <X className="w-3 h-3 mr-1" />
          Cancelled
        </Badge>
      );
    default:
      return (
        <Badge variant="outline">
          <AlertCircle className="w-3 h-3 mr-1" />
          {status}
        </Badge>
      );
  }
};

const formatDate = (dateStr: string | null): string => {
  if (!dateStr) return 'N/A';
  try {
    return new Date(dateStr).toLocaleString();
  } catch {
    return dateStr;
  }
};

const JobsPanel: React.FC = () => {
  const { accounts, currentAccount } = useAccount();
  const { data: jobsData, isLoading: jobsLoading, refetch } = useJobs({ limit: 50 });
  const cancelJobMutation = useCancelJob();
  const startJobMutation = useStartProcessEmailsJob();

  // Form state for new job
  const [newJobInstruction, setNewJobInstruction] = useState('');
  const [newJobAccountId, setNewJobAccountId] = useState(currentAccount?.id || '');
  const [newJobFolder, setNewJobFolder] = useState('INBOX');
  const [selectedJob, setSelectedJob] = useState<Job | null>(null);

  const jobs = jobsData?.jobs || [];

  const handleStartJob = async () => {
    if (!newJobInstruction || !newJobAccountId) return;

    try {
      await startJobMutation.mutateAsync({
        instruction: newJobInstruction,
        account_id: newJobAccountId,
        folder: newJobFolder || undefined,
      });
      setNewJobInstruction('');
    } catch (error) {
      console.error('Failed to start job:', error);
    }
  };

  const handleCancelJob = async (jobId: string) => {
    try {
      await cancelJobMutation.mutateAsync(jobId);
    } catch (error) {
      console.error('Failed to cancel job:', error);
    }
  };

  return (
    <div className="h-full flex flex-col gap-4 overflow-hidden p-4">
      {/* New Job Form */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-lg">Start New Job</CardTitle>
          <CardDescription>
            Start an AI-powered email processing job with natural language instructions
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="instruction">Instruction</Label>
              <Input
                id="instruction"
                placeholder="e.g., Move all unread emails from newsletters to Archive folder"
                value={newJobInstruction}
                onChange={(e) => setNewJobInstruction(e.target.value)}
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label htmlFor="account">Account</Label>
                <Select
                  value={newJobAccountId}
                  onValueChange={setNewJobAccountId}
                >
                  <SelectTrigger id="account">
                    <SelectValue placeholder="Select account" />
                  </SelectTrigger>
                  <SelectContent>
                    {accounts.map((account) => (
                      <SelectItem key={account.id} value={account.id}>
                        {account.email_address}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-2">
                <Label htmlFor="folder">Folder (optional)</Label>
                <Input
                  id="folder"
                  placeholder="INBOX"
                  value={newJobFolder}
                  onChange={(e) => setNewJobFolder(e.target.value)}
                />
              </div>
            </div>
            <Button
              onClick={handleStartJob}
              disabled={!newJobInstruction || !newJobAccountId || startJobMutation.isPending}
              className="w-full"
            >
              {startJobMutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Starting...
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 mr-2" />
                  Start Job
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Jobs List */}
      <Card className="flex-1 flex flex-col overflow-hidden">
        <CardHeader className="pb-3 flex flex-row items-center justify-between">
          <div>
            <CardTitle className="text-lg">Background Jobs</CardTitle>
            <CardDescription>
              {jobs.length} job{jobs.length !== 1 ? 's' : ''} total
            </CardDescription>
          </div>
          <Button variant="outline" size="sm" onClick={() => refetch()}>
            <RefreshCw className="w-4 h-4 mr-2" />
            Refresh
          </Button>
        </CardHeader>
        <CardContent className="flex-1 overflow-auto">
          {jobsLoading ? (
            <div className="flex items-center justify-center h-32">
              <Loader2 className="w-8 h-8 animate-spin text-muted-foreground" />
            </div>
          ) : jobs.length === 0 ? (
            <div className="flex items-center justify-center h-32 text-muted-foreground">
              No jobs found
            </div>
          ) : (
            <div className="space-y-3">
              {jobs.map((job) => (
                <div
                  key={job.job_id}
                  className={`
                    p-3 border rounded-lg cursor-pointer transition-colors
                    ${selectedJob?.job_id === job.job_id ? 'border-primary bg-muted/50' : 'hover:bg-muted/30'}
                  `}
                  onClick={() => setSelectedJob(selectedJob?.job_id === job.job_id ? null : job)}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <JobStatusBadge status={job.status} />
                        <span className="text-xs text-muted-foreground font-mono truncate">
                          {job.job_id}
                        </span>
                      </div>
                      <p className="text-sm truncate">
                        {job.instruction || 'No instruction'}
                      </p>
                      <div className="flex items-center gap-4 mt-1 text-xs text-muted-foreground">
                        <span className="flex items-center gap-1">
                          <Clock className="w-3 h-3" />
                          Started: {formatDate(job.started_at)}
                        </span>
                        {job.completed_at && (
                          <span>
                            Completed: {formatDate(job.completed_at)}
                          </span>
                        )}
                      </div>
                    </div>
                    {job.status === 'running' && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleCancelJob(job.job_id);
                        }}
                        disabled={cancelJobMutation.isPending}
                      >
                        {cancelJobMutation.isPending ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <X className="w-4 h-4" />
                        )}
                      </Button>
                    )}
                  </div>

                  {/* Expanded details */}
                  {selectedJob?.job_id === job.job_id && (
                    <div className="mt-3 pt-3 border-t space-y-2 text-sm">
                      {job.error_message && (
                        <div className="p-2 bg-destructive/10 rounded text-destructive">
                          <strong>Error:</strong> {job.error_message}
                        </div>
                      )}
                      {job.result_data && (
                        <div className="p-2 bg-muted rounded">
                          <strong>Result:</strong>
                          <pre className="mt-1 text-xs overflow-auto max-h-40 whitespace-pre-wrap">
                            {job.result_data}
                          </pre>
                        </div>
                      )}
                      <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                        <div>Resumable: {job.resumable ? 'Yes' : 'No'}</div>
                        <div>Retries: {job.retry_count} / {job.max_retries}</div>
                        <div>Updated: {formatDate(job.updated_at)}</div>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default JobsPanel;
