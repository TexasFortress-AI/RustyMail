
import React, { useState, useRef, useEffect } from 'react';
import { useSSEEvents } from '@/dashboard/hooks/useSSEEvents';
import { GripHorizontal } from 'lucide-react';
import TopBar from './TopBar';
import StatsPanel from './StatsPanel';
import ClientListPanel from './ClientListPanel';
import ChatbotPanel from './ChatbotPanel';
import McpTools from './McpTools';

const Dashboard: React.FC = () => {
  // Initialize SSE event listener
  useSSEEvents();

  // Splitter state
  const [topHeight, setTopHeight] = useState(60); // percentage for top section
  const [isResizing, setIsResizing] = useState(false);
  const resizeStartRef = useRef({ clientY: 0, startTopHeight: 0, containerHeight: 0 });

  // Handle splitter resize
  const handleResizeStart = (e: React.MouseEvent) => {
    console.log('handleResizeStart called', e.clientY);
    e.preventDefault();
    const container = document.querySelector('[data-testid="dashboard-container"]') as HTMLElement;
    if (!container) {
      console.error('Dashboard container not found');
      return;
    }

    const containerRect = container.getBoundingClientRect();
    console.log('Container rect:', containerRect);
    setIsResizing(true);
    resizeStartRef.current = {
      clientY: e.clientY,
      startTopHeight: topHeight,
      containerHeight: containerRect.height
    };
    console.log('Resize started:', resizeStartRef.current);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      console.log('handleMouseMove called', e.clientY);

      const container = document.querySelector('[data-testid="dashboard-container"]') as HTMLElement;
      if (!container) {
        console.error('Container not found in mousemove');
        return;
      }

      const containerRect = container.getBoundingClientRect();
      const deltaY = e.clientY - resizeStartRef.current.clientY;
      const deltaPercentage = (deltaY / containerRect.height) * 100;
      const newTopHeight = Math.max(20, Math.min(80, resizeStartRef.current.startTopHeight + deltaPercentage));
      console.log('Setting topHeight to:', newTopHeight, 'deltaY:', deltaY, 'deltaPercentage:', deltaPercentage);
      setTopHeight(newTopHeight);
    };

    const handleMouseUp = () => {
      console.log('handleMouseUp called');
      setIsResizing(false);
    };

    if (isResizing) {
      console.log('Adding mouse event listeners');
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = 'ns-resize';
      document.body.style.userSelect = 'none';
    } else {
      console.log('Removing mouse event listeners');
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
      className="h-screen bg-gradient-to-br from-background to-secondary/30 flex flex-col"
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
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1 min-h-0 overflow-auto">
            <div className="min-h-0 overflow-auto">
              <StatsPanel />
            </div>
            <div className="min-h-0 overflow-auto">
              <ClientListPanel />
            </div>
            <div className="min-h-0 overflow-auto">
              <McpTools />
            </div>
          </div>
        </div>

        {/* Splitter */}
        <div
          className={`
            relative h-4 py-1 bg-muted/30 cursor-ns-resize flex items-center justify-center
            hover:bg-muted/50 transition-colors duration-150 select-none
            ${isResizing ? 'bg-primary/20' : ''}
          `}
          onMouseDown={handleResizeStart}
          title="Drag to resize panels"
        >
          <GripHorizontal className="h-3 w-3 text-muted-foreground pointer-events-none" />
          {/* Debug info */}
          <div className="absolute right-2 text-xs text-muted-foreground">
            {topHeight.toFixed(0)}% | {isResizing ? 'RESIZING' : 'IDLE'}
          </div>
          {/* Debug test buttons */}
          <div className="absolute left-2 flex gap-1">
            <button
              onClick={() => setTopHeight(20)}
              className="text-xs bg-blue-500 text-white px-1 rounded"
            >
              20%
            </button>
            <button
              onClick={() => setTopHeight(80)}
              className="text-xs bg-green-500 text-white px-1 rounded"
            >
              80%
            </button>
          </div>
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
