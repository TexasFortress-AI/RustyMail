
import React, { useState, useRef, useEffect } from 'react';
import { useSSEEvents } from '@/dashboard/hooks/useSSEEvents';
import { GripHorizontal } from 'lucide-react';
import TopBar from './TopBar';
import StatsPanel from './StatsPanel';
import ClientListPanel from './ClientListPanel';
import ChatbotPanel from './ChatbotPanel';

const Dashboard: React.FC = () => {
  // Initialize SSE event listener
  useSSEEvents();

  // Splitter state
  const [topHeight, setTopHeight] = useState(60); // percentage for top section
  const [isResizing, setIsResizing] = useState(false);
  const resizeStartRef = useRef({ clientY: 0, startTopHeight: 0, containerHeight: 0 });

  // Handle splitter resize
  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const container = document.querySelector('[data-testid="dashboard-container"]') as HTMLElement;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    setIsResizing(true);
    resizeStartRef.current = {
      clientY: e.clientY,
      startTopHeight: topHeight,
      containerHeight: containerRect.height
    };
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;

      const container = document.querySelector('[data-testid="dashboard-container"]') as HTMLElement;
      if (!container) return;

      const containerRect = container.getBoundingClientRect();
      const deltaY = e.clientY - resizeStartRef.current.clientY;
      const deltaPercentage = (deltaY / containerRect.height) * 100;
      const newTopHeight = Math.max(20, Math.min(80, resizeStartRef.current.startTopHeight + deltaPercentage));
      setTopHeight(newTopHeight);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = 'ns-resize';
      document.body.style.userSelect = 'none';
    } else {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizing, topHeight]);

  return (
    <div
      className="min-h-screen bg-gradient-to-br from-background to-secondary/30 flex flex-col"
      data-testid="dashboard-layout"
    >
      <TopBar />

      <div
        className="container mx-auto px-4 py-6 flex-1 flex flex-col"
        data-testid="dashboard-container"
      >
        {/* Top section - Widgets */}
        <div
          className="overflow-hidden flex flex-col min-h-0"
          style={{ height: `${topHeight}%` }}
        >
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 flex-1 min-h-0 overflow-auto">
            <div className="min-h-0 overflow-auto">
              <StatsPanel />
            </div>
            <div className="min-h-0 overflow-auto">
              <ClientListPanel />
            </div>
          </div>
        </div>

        {/* Splitter */}
        <div
          className={`
            relative h-2 bg-muted/30 cursor-ns-resize flex items-center justify-center
            hover:bg-muted/50 transition-colors duration-150
            ${isResizing ? 'bg-primary/20' : ''}
          `}
          onMouseDown={handleResizeStart}
          title="Drag to resize panels"
        >
          <GripHorizontal className="h-3 w-3 text-muted-foreground" />
        </div>

        {/* Bottom section - Chatbot */}
        <div
          className="overflow-hidden flex flex-col min-h-0"
          style={{ height: `${100 - topHeight}%` }}
        >
          <div className="flex-1 min-h-0">
            <ChatbotPanel />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
