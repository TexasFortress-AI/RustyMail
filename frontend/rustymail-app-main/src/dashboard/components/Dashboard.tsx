
import React, { useState, useRef, useEffect } from 'react';
import { useSSEEvents } from '@/dashboard/hooks/useSSEEvents';
import { GripHorizontal } from 'lucide-react';
import TopBar from './TopBar';
import StatsPanel from './StatsPanel';
import ClientListPanel from './ClientListPanel';
import ChatbotPanel from './ChatbotPanel';
import McpTools from './McpTools';
import EmailList from './EmailList';

const Dashboard: React.FC = () => {
  // Initialize SSE event listener
  useSSEEvents();

  // Splitter state
  const [topHeight, setTopHeight] = useState(60); // percentage for top section
  const [isResizing, setIsResizing] = useState(false);
  const resizeStartRef = useRef({ clientY: 0, startTopHeight: 0, containerHeight: 0 });

  // Right panel splitter states (percentages)
  const [rightPanel1Height, setRightPanel1Height] = useState(25); // StatsPanel height
  const [rightPanel2Height, setRightPanel2Height] = useState(37.5); // ClientListPanel height
  // McpTools gets the remaining height (100 - panel1 - panel2)
  const [isResizingRight1, setIsResizingRight1] = useState(false);
  const [isResizingRight2, setIsResizingRight2] = useState(false);
  const rightResizeStartRef = useRef({ clientY: 0, startHeight: 0, containerHeight: 0, panelIndex: 0 });

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

  // Handle right panel splitter resize
  const handleRightResizeStart = (e: React.MouseEvent, panelIndex: number) => {
    e.preventDefault();
    e.stopPropagation();
    const container = document.querySelector('[data-testid="right-panels-container"]') as HTMLElement;
    if (!container) {
      console.error('Right panels container not found');
      return;
    }

    const containerRect = container.getBoundingClientRect();
    if (panelIndex === 1) {
      setIsResizingRight1(true);
      rightResizeStartRef.current = {
        clientY: e.clientY,
        startHeight: rightPanel1Height,
        containerHeight: containerRect.height,
        panelIndex: 1
      };
    } else if (panelIndex === 2) {
      setIsResizingRight2(true);
      rightResizeStartRef.current = {
        clientY: e.clientY,
        startHeight: rightPanel2Height,
        containerHeight: containerRect.height,
        panelIndex: 2
      };
    }
  };

  useEffect(() => {
    const handleRightMouseMove = (e: MouseEvent) => {
      if (!isResizingRight1 && !isResizingRight2) return;

      const container = document.querySelector('[data-testid="right-panels-container"]') as HTMLElement;
      if (!container) return;

      const containerRect = container.getBoundingClientRect();
      const deltaY = e.clientY - rightResizeStartRef.current.clientY;
      const deltaPercentage = (deltaY / containerRect.height) * 100;

      if (isResizingRight1) {
        // Resizing panel 1 affects panel 1 and panel 2
        const newPanel1Height = Math.max(10, Math.min(60, rightResizeStartRef.current.startHeight + deltaPercentage));
        const totalAvailable = 100 - newPanel1Height;
        const panel2Ratio = rightPanel2Height / (rightPanel2Height + (100 - rightPanel1Height - rightPanel2Height));
        const newPanel2Height = Math.max(10, Math.min(60, totalAvailable * panel2Ratio));

        setRightPanel1Height(newPanel1Height);
        setRightPanel2Height(newPanel2Height);
      } else if (isResizingRight2) {
        // Resizing panel 2 only affects panel 2 and panel 3
        const maxPanel2 = 100 - rightPanel1Height - 10; // Leave at least 10% for panel 3
        const newPanel2Height = Math.max(10, Math.min(maxPanel2, rightResizeStartRef.current.startHeight + deltaPercentage));
        setRightPanel2Height(newPanel2Height);
      }
    };

    const handleRightMouseUp = () => {
      setIsResizingRight1(false);
      setIsResizingRight2(false);
    };

    if (isResizingRight1 || isResizingRight2) {
      document.addEventListener('mousemove', handleRightMouseMove);
      document.addEventListener('mouseup', handleRightMouseUp);
      document.body.style.cursor = 'ns-resize';
      document.body.style.userSelect = 'none';
    } else {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    return () => {
      document.removeEventListener('mousemove', handleRightMouseMove);
      document.removeEventListener('mouseup', handleRightMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizingRight1, isResizingRight2, rightPanel1Height, rightPanel2Height]);

  return (
    <div
      className="h-screen bg-gradient-to-br from-background to-secondary/30 flex flex-col"
      data-testid="dashboard-layout"
    >
      <TopBar />

      <div
        className="container mx-auto px-4 py-6 flex-1 flex flex-col h-full"
        data-testid="dashboard-container"
      >
        {/* Top section - Widgets */}
        <div
          className="flex flex-col min-h-0"
          style={{ height: `${topHeight}%` }}
        >
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 flex-1 overflow-hidden">
            <div className="flex flex-col min-h-0">
              <EmailList />
            </div>
            <div className="flex flex-col min-h-0" data-testid="right-panels-container">
              {/* StatsPanel */}
              <div
                className="overflow-hidden"
                style={{ height: `${rightPanel1Height}%` }}
              >
                <StatsPanel />
              </div>

              {/* First Splitter */}
              <div
                className={`
                  relative h-3 py-0.5 bg-muted/30 cursor-ns-resize flex items-center justify-center
                  hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
                  ${isResizingRight1 ? 'bg-primary/20' : ''}
                `}
                onMouseDown={(e) => handleRightResizeStart(e, 1)}
                title="Drag to resize panels"
              >
                <GripHorizontal className="h-2 w-2 text-muted-foreground pointer-events-none" />
              </div>

              {/* ClientListPanel */}
              <div
                className="overflow-hidden"
                style={{ height: `${rightPanel2Height}%` }}
              >
                <ClientListPanel />
              </div>

              {/* Second Splitter */}
              <div
                className={`
                  relative h-3 py-0.5 bg-muted/30 cursor-ns-resize flex items-center justify-center
                  hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
                  ${isResizingRight2 ? 'bg-primary/20' : ''}
                `}
                onMouseDown={(e) => handleRightResizeStart(e, 2)}
                title="Drag to resize panels"
              >
                <GripHorizontal className="h-2 w-2 text-muted-foreground pointer-events-none" />
              </div>

              {/* McpTools */}
              <div
                className="flex-1 min-h-0 overflow-hidden"
                style={{ height: `calc(${100 - rightPanel1Height - rightPanel2Height}% - 1.5rem)` }}
              >
                <McpTools />
              </div>
            </div>
          </div>
        </div>

        {/* Splitter */}
        <div
          className={`
            relative h-4 py-1 bg-muted/30 cursor-ns-resize flex items-center justify-center
            hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
            ${isResizing ? 'bg-primary/20' : ''}
          `}
          onMouseDown={handleResizeStart}
          title="Drag to resize panels"
        >
          <GripHorizontal className="h-3 w-3 text-muted-foreground pointer-events-none" />
        </div>

        {/* Bottom section - Chatbot */}
        <div
          className="flex flex-col min-h-0"
          style={{ height: `calc(${100 - topHeight}% - 1rem)` }}
        >
          <ChatbotPanel />
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
