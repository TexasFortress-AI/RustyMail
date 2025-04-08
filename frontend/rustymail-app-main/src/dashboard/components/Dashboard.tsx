
import React from 'react';
import { useSSEEvents } from '@/dashboard/hooks/useSSEEvents';
import TopBar from './TopBar';
import StatsPanel from './StatsPanel';
import ClientListPanel from './ClientListPanel';
import ChatbotPanel from './ChatbotPanel';

const Dashboard: React.FC = () => {
  // Initialize SSE event listener
  useSSEEvents();

  return (
    <div 
      className="min-h-screen bg-gradient-to-br from-background to-secondary/30 flex flex-col"
      data-testid="dashboard-layout"
    >
      <TopBar />
      
      <div className="container mx-auto px-4 py-6 flex-1 flex flex-col gap-6">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <StatsPanel />
          <ClientListPanel />
        </div>
        
        <div className="mt-auto">
          <ChatbotPanel />
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
