
import React, { useState, useRef, useEffect } from 'react';
import { useSSEEvents } from '@/dashboard/hooks/useSSEEvents';
import { GripHorizontal } from 'lucide-react';
import TopBar from './TopBar';
import StatsPanel from './StatsPanel';
import ClientListPanel from './ClientListPanel';
import ChatbotPanel from './ChatbotPanel';
import McpTools from './McpTools';
import EmailList from './EmailList';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

const Dashboard: React.FC = () => {
  // Initialize SSE event listener
  useSSEEvents();

  // Active tab state
  const [activeTab, setActiveTab] = useState('email');

  // Splitter state for Email tab (vertical - top/bottom)
  const [emailTopHeight, setEmailTopHeight] = useState(60); // percentage for top section
  const [isResizingEmail, setIsResizingEmail] = useState(false);
  const emailResizeStartRef = useRef({ clientY: 0, startTopHeight: 0, containerHeight: 0 });

  // Splitter state for Email top section (horizontal - EmailList/McpTools)
  const [emailListWidth, setEmailListWidth] = useState(50); // percentage for EmailList
  const [isResizingEmailTop, setIsResizingEmailTop] = useState(false);
  const emailTopResizeStartRef = useRef({ clientX: 0, startLeftWidth: 0, containerWidth: 0 });

  // Splitter state for System tab
  const [systemLeftWidth, setSystemLeftWidth] = useState(50); // percentage for left panel
  const [isResizingSystem, setIsResizingSystem] = useState(false);
  const systemResizeStartRef = useRef({ clientX: 0, startLeftWidth: 0, containerWidth: 0 });

  // Handle Email tab vertical splitter resize
  const handleEmailResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const container = document.querySelector('[data-testid="email-tab-container"]') as HTMLElement;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    setIsResizingEmail(true);
    emailResizeStartRef.current = {
      clientY: e.clientY,
      startTopHeight: emailTopHeight,
      containerHeight: containerRect.height
    };
  };

  useEffect(() => {
    const handleEmailMouseMove = (e: MouseEvent) => {
      if (!isResizingEmail) return;

      const container = document.querySelector('[data-testid="email-tab-container"]') as HTMLElement;
      if (!container) return;

      const containerRect = container.getBoundingClientRect();
      const deltaY = e.clientY - emailResizeStartRef.current.clientY;
      const deltaPercentage = (deltaY / containerRect.height) * 100;
      const newTopHeight = Math.max(20, Math.min(80, emailResizeStartRef.current.startTopHeight + deltaPercentage));
      setEmailTopHeight(newTopHeight);
    };

    const handleEmailMouseUp = () => {
      setIsResizingEmail(false);
    };

    if (isResizingEmail) {
      document.addEventListener('mousemove', handleEmailMouseMove);
      document.addEventListener('mouseup', handleEmailMouseUp);
      document.body.style.cursor = 'ns-resize';
      document.body.style.userSelect = 'none';
    } else {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    return () => {
      document.removeEventListener('mousemove', handleEmailMouseMove);
      document.removeEventListener('mouseup', handleEmailMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizingEmail, emailTopHeight]);

  // Handle Email top section horizontal splitter resize
  const handleEmailTopResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const container = e.currentTarget.parentElement as HTMLElement;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    setIsResizingEmailTop(true);
    emailTopResizeStartRef.current = {
      clientX: e.clientX,
      startLeftWidth: emailListWidth,
      containerWidth: containerRect.width
    };
  };

  useEffect(() => {
    const handleEmailTopMouseMove = (e: MouseEvent) => {
      if (!isResizingEmailTop) return;

      const deltaX = e.clientX - emailTopResizeStartRef.current.clientX;
      const deltaPercentage = (deltaX / emailTopResizeStartRef.current.containerWidth) * 100;
      const newLeftWidth = Math.max(30, Math.min(70, emailTopResizeStartRef.current.startLeftWidth + deltaPercentage));
      setEmailListWidth(newLeftWidth);
    };

    const handleEmailTopMouseUp = () => {
      setIsResizingEmailTop(false);
    };

    if (isResizingEmailTop) {
      document.addEventListener('mousemove', handleEmailTopMouseMove);
      document.addEventListener('mouseup', handleEmailTopMouseUp);
      document.body.style.cursor = 'ew-resize';
      document.body.style.userSelect = 'none';
    } else {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    return () => {
      document.removeEventListener('mousemove', handleEmailTopMouseMove);
      document.removeEventListener('mouseup', handleEmailTopMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizingEmailTop]);

  // Handle System tab horizontal splitter resize
  const handleSystemResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const container = document.querySelector('[data-testid="system-tab-container"]') as HTMLElement;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    setIsResizingSystem(true);
    systemResizeStartRef.current = {
      clientX: e.clientX,
      startLeftWidth: systemLeftWidth,
      containerWidth: containerRect.width
    };
  };

  useEffect(() => {
    const handleSystemMouseMove = (e: MouseEvent) => {
      if (!isResizingSystem) return;

      const container = document.querySelector('[data-testid="system-tab-container"]') as HTMLElement;
      if (!container) return;

      const containerRect = container.getBoundingClientRect();
      const deltaX = e.clientX - systemResizeStartRef.current.clientX;
      const deltaPercentage = (deltaX / containerRect.width) * 100;
      const newLeftWidth = Math.max(30, Math.min(70, systemResizeStartRef.current.startLeftWidth + deltaPercentage));
      setSystemLeftWidth(newLeftWidth);
    };

    const handleSystemMouseUp = () => {
      setIsResizingSystem(false);
    };

    if (isResizingSystem) {
      document.addEventListener('mousemove', handleSystemMouseMove);
      document.addEventListener('mouseup', handleSystemMouseUp);
      document.body.style.cursor = 'ew-resize';
      document.body.style.userSelect = 'none';
    } else {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    return () => {
      document.removeEventListener('mousemove', handleSystemMouseMove);
      document.removeEventListener('mouseup', handleSystemMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizingSystem, systemLeftWidth]);

  return (
    <div
      className="h-screen bg-gradient-to-br from-background to-secondary/30 flex flex-col"
      data-testid="dashboard-layout"
    >
      <TopBar />

      <div
        className="container mx-auto px-4 py-6 flex-1 flex flex-col h-full min-h-0"
        data-testid="dashboard-container"
      >
        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 flex flex-col min-h-0">
          <TabsList className="grid w-full max-w-md grid-cols-2 mb-4">
            <TabsTrigger value="email">Email</TabsTrigger>
            <TabsTrigger value="system">System</TabsTrigger>
          </TabsList>

          {/* Email Tab */}
          <TabsContent value="email" className="flex-1 flex flex-col min-h-0 mt-0 data-[state=inactive]:hidden" data-testid="email-tab-container">
            {/* Top section - EmailList and McpTools */}
            <div
              className="flex gap-6 overflow-hidden min-h-0"
              style={{ height: `${emailTopHeight}%` }}
            >
              {/* EmailList */}
              <div
                className="overflow-hidden min-h-0"
                style={{ width: `${emailListWidth}%` }}
              >
                <EmailList />
              </div>

              {/* Horizontal Splitter between EmailList and McpTools */}
              <div
                className={`
                  relative w-4 px-1 bg-muted/30 cursor-ew-resize flex items-center justify-center
                  hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
                  ${isResizingEmailTop ? 'bg-primary/20' : ''}
                `}
                onMouseDown={handleEmailTopResizeStart}
                title="Drag to resize panels"
              >
                <div className="h-full w-px bg-muted-foreground/20" />
              </div>

              {/* McpTools */}
              <div
                className="overflow-hidden min-h-0"
                style={{ width: `${100 - emailListWidth}%` }}
              >
                <McpTools />
              </div>
            </div>

            {/* Vertical Splitter between top and bottom */}
            <div
              className={`
                relative h-4 py-1 bg-muted/30 cursor-ns-resize flex items-center justify-center
                hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
                ${isResizingEmail ? 'bg-primary/20' : ''}
              `}
              onMouseDown={handleEmailResizeStart}
              title="Drag to resize panels"
            >
              <GripHorizontal className="h-3 w-3 text-muted-foreground pointer-events-none" />
            </div>

            {/* Bottom section - Chatbot */}
            <div
              className="overflow-hidden min-h-0"
              style={{ height: `${100 - emailTopHeight}%` }}
            >
              <ChatbotPanel />
            </div>
          </TabsContent>

          {/* System Tab */}
          <TabsContent value="system" className="flex-1 flex gap-6 min-h-0 mt-0 data-[state=inactive]:hidden" data-testid="system-tab-container">
            {/* Left panel - StatsPanel */}
            <div className="flex-1 overflow-hidden min-h-0">
              <StatsPanel />
            </div>

            {/* Vertical Splitter */}
            <div
              className={`
                relative w-4 px-1 bg-muted/30 cursor-ew-resize flex items-center justify-center
                hover:bg-muted/50 transition-colors duration-150 select-none flex-shrink-0
                ${isResizingSystem ? 'bg-primary/20' : ''}
              `}
              onMouseDown={handleSystemResizeStart}
              title="Drag to resize panels"
            >
              <div className="h-full w-px bg-muted-foreground/20" />
            </div>

            {/* Right panel - ClientListPanel */}
            <div className="flex-1 overflow-hidden min-h-0">
              <ClientListPanel />
            </div>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
};

export default Dashboard;
