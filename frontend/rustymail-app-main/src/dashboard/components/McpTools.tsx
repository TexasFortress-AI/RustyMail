// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import React, { useEffect, useState } from 'react';
import { ChevronDown, ChevronRight, Play, Terminal, Code, X, Copy, Check, Layers, Zap } from 'lucide-react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import config from '../config';
import { useAccount } from '../../contexts/AccountContext';
import type { EmailContext } from './EmailList';

interface McpTool {
  name: string;
  description: string;
  parameters: { [key: string]: string };
}

interface McpToolsProps {
  currentFolder?: string;
  selectedEmailContext?: EmailContext;
}

const McpTools: React.FC<McpToolsProps> = ({ currentFolder, selectedEmailContext }) => {
  const { currentAccount } = useAccount();
  const [lowLevelTools, setLowLevelTools] = useState<McpTool[]>([]);
  const [highLevelTools, setHighLevelTools] = useState<McpTool[]>([]);
  const [activeTab, setActiveTab] = useState<'low-level' | 'high-level'>('low-level');
  const [expandedTool, setExpandedTool] = useState<string | null>(null);
  const [executing, setExecuting] = useState<string | null>(null);
  const [results, setResults] = useState<{ [key: string]: any }>({});
  const [parameters, setParameters] = useState<{ [key: string]: { [key: string]: string } }>({});
  const [error, setError] = useState<string | null>(null);
  const [copiedTool, setCopiedTool] = useState<string | null>(null);

  // Get the active tools based on current tab
  const tools = activeTab === 'low-level' ? lowLevelTools : highLevelTools;

  // Get context-based default values for parameters
  const getContextDefaults = (paramName: string): string => {
    const lowerParam = paramName.toLowerCase();

    // Account ID
    if (lowerParam === 'account_id' && currentAccount) {
      return currentAccount.email_address || currentAccount.id;
    }

    // Folder
    if (lowerParam === 'folder' && currentFolder) {
      return currentFolder;
    }

    // UID
    if (lowerParam === 'uid' && selectedEmailContext?.uid !== undefined) {
      return selectedEmailContext.uid.toString();
    }

    // Message ID
    if (lowerParam === 'message_id' && selectedEmailContext?.message_id) {
      return selectedEmailContext.message_id;
    }

    // Index
    if (lowerParam === 'index' && selectedEmailContext?.index !== undefined) {
      return selectedEmailContext.index.toString();
    }

    return '';
  };

  useEffect(() => {
    fetchTools();
  }, []);

  // Update parameters when context changes (for expanded tool)
  useEffect(() => {
    if (expandedTool && tools.length > 0) {
      const tool = tools.find(t => t.name === expandedTool);
      if (tool) {
        const updatedParams: { [key: string]: string } = {};
        Object.keys(tool.parameters).forEach(paramName => {
          const defaultValue = getContextDefaults(paramName);
          if (defaultValue) {
            updatedParams[paramName] = defaultValue;
          }
        });

        // Only update if we have values to set
        if (Object.keys(updatedParams).length > 0) {
          setParameters(prev => ({
            ...prev,
            [expandedTool]: {
              ...prev[expandedTool],
              ...updatedParams
            }
          }));
        }
      }
    }
  }, [currentFolder, selectedEmailContext, currentAccount, expandedTool, tools]);

  const fetchTools = async () => {
    try {
      // Fetch both low-level and high-level tools
      const [lowLevelResponse, highLevelResponse] = await Promise.all([
        fetch(`${config.api.baseUrl}/dashboard/mcp/tools?variant=standard`, {
          headers: { 'X-API-Key': config.api.apiKey }
        }),
        fetch(`${config.api.baseUrl}/dashboard/mcp/tools?variant=high-level`, {
          headers: { 'X-API-Key': config.api.apiKey }
        })
      ]);

      if (!lowLevelResponse.ok || !highLevelResponse.ok) {
        throw new Error(`Failed to fetch tools`);
      }

      const lowLevelData = await lowLevelResponse.json();
      const highLevelData = await highLevelResponse.json();

      setLowLevelTools(lowLevelData.tools || []);
      setHighLevelTools(highLevelData.tools || []);

      // Initialize parameters state for all tools
      const initialParams: { [key: string]: { [key: string]: string } } = {};
      [...(lowLevelData.tools || []), ...(highLevelData.tools || [])].forEach((tool: McpTool) => {
        initialParams[tool.name] = {};
        Object.keys(tool.parameters).forEach(param => {
          initialParams[tool.name][param] = '';
        });
      });
      setParameters(initialParams);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch MCP tools');
      console.error('Error fetching MCP tools:', err);
    }
  };

  const executeTool = async (toolName: string) => {
    setExecuting(toolName);
    setResults(prev => ({ ...prev, [toolName]: null }));

    try {
      // Use user-provided parameters, only default account_id if not specified
      const userParams = parameters[toolName] || {};
      const toolParameters = {
        ...(currentAccount && !userParams.account_id ? { account_id: currentAccount.id } : {}),
        ...userParams
      };

      const variant = activeTab === 'high-level' ? 'high-level' : 'standard';
      const response = await fetch(`${config.api.baseUrl}/dashboard/mcp/execute?variant=${variant}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-API-Key': config.api.apiKey
        },
        body: JSON.stringify({
          tool: toolName,
          parameters: toolParameters
        })
      });

      const result = await response.json();
      setResults(prev => ({ ...prev, [toolName]: result }));
    } catch (err) {
      setResults(prev => ({
        ...prev,
        [toolName]: { error: err instanceof Error ? err.message : 'Execution failed' }
      }));
    } finally {
      setExecuting(null);
    }
  };

  const toggleTool = (toolName: string) => {
    const isExpanding = expandedTool !== toolName;
    setExpandedTool(isExpanding ? toolName : null);

    // Auto-fill parameters when expanding
    if (isExpanding) {
      const tool = tools.find(t => t.name === toolName);
      if (tool) {
        const autoFilledParams: { [key: string]: string } = {};
        Object.keys(tool.parameters).forEach(paramName => {
          const defaultValue = getContextDefaults(paramName);
          if (defaultValue) {
            autoFilledParams[paramName] = defaultValue;
          }
        });

        // Merge auto-filled with existing parameters
        setParameters(prev => ({
          ...prev,
          [toolName]: {
            ...prev[toolName],
            ...autoFilledParams
          }
        }));
      }
    }
  };

  const updateParameter = (toolName: string, paramName: string, value: string) => {
    setParameters(prev => ({
      ...prev,
      [toolName]: {
        ...prev[toolName],
        [paramName]: value
      }
    }));
  };

  const clearParameter = (toolName: string, paramName: string) => {
    setParameters(prev => ({
      ...prev,
      [toolName]: {
        ...prev[toolName],
        [paramName]: ''
      }
    }));
  };

  const copyResult = async (toolName: string) => {
    try {
      const resultText = JSON.stringify(results[toolName], null, 2);
      await navigator.clipboard.writeText(resultText);
      setCopiedTool(toolName);
      setTimeout(() => setCopiedTool(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  return (
    <div className="bg-card border rounded-lg p-4 h-full flex flex-col">
      <div className="flex items-center gap-2 mb-4 flex-shrink-0">
        <Terminal className="w-5 h-5 text-primary" />
        <h3 className="text-lg font-semibold">MCP Email Tools</h3>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/50 rounded p-3 mb-4 flex-shrink-0">
          <p className="text-destructive text-sm">{error}</p>
        </div>
      )}

      <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as 'low-level' | 'high-level')} className="flex-1 flex flex-col">
        <TabsList className="grid w-full grid-cols-2 mb-4">
          <TabsTrigger value="low-level" className="flex items-center gap-2">
            <Layers className="w-4 h-4" />
            Low-Level ({lowLevelTools.length})
          </TabsTrigger>
          <TabsTrigger value="high-level" className="flex items-center gap-2">
            <Zap className="w-4 h-4" />
            High-Level ({highLevelTools.length})
          </TabsTrigger>
        </TabsList>

        <TabsContent value={activeTab} className="flex-1 overflow-y-auto mt-0">
          <div className="space-y-2">
            {tools.map(tool => (
          <div key={tool.name} className="border rounded overflow-hidden">
            {/* Tool Header */}
            <button
              onClick={() => toggleTool(tool.name)}
              className="w-full flex items-center gap-2 p-3 bg-muted hover:bg-muted/80 transition-colors"
            >
              {expandedTool === tool.name ?
                <ChevronDown className="w-4 h-4 text-muted-foreground" /> :
                <ChevronRight className="w-4 h-4 text-muted-foreground" />
              }
              <Code className="w-4 h-4 text-primary" />
              <span className="font-mono text-sm">{tool.name}</span>
              <span className="text-muted-foreground text-xs ml-auto">{tool.description}</span>
            </button>

            {/* Tool Body */}
            {expandedTool === tool.name && (
              <div className="p-4 bg-muted/50 border-t">
                {/* Parameters */}
                {Object.keys(tool.parameters).length > 0 && (
                  <div className="mb-4">
                    <h4 className="text-xs font-semibold text-muted-foreground mb-2">PARAMETERS</h4>
                    <div className="space-y-2">
                      {Object.entries(tool.parameters).map(([paramName, paramDesc]) => (
                        <div key={paramName}>
                          <label className="block text-xs text-muted-foreground mb-1">
                            {paramName}: <span className="text-muted-foreground/70">{paramDesc}</span>
                          </label>
                          <div className="relative">
                            <input
                              type="text"
                              value={parameters[tool.name]?.[paramName] || ''}
                              onChange={(e) => updateParameter(tool.name, paramName, e.target.value)}
                              className="w-full px-2 py-1 pr-8 bg-background border rounded text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                              placeholder={`Enter ${paramName}`}
                            />
                            {parameters[tool.name]?.[paramName] && (
                              <button
                                onClick={() => clearParameter(tool.name, paramName)}
                                className="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 hover:bg-muted rounded"
                                title="Clear field"
                              >
                                <X className="w-3 h-3 text-muted-foreground hover:text-foreground" />
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Execute Button */}
                <button
                  onClick={() => executeTool(tool.name)}
                  disabled={executing === tool.name}
                  className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 disabled:bg-muted disabled:opacity-50 text-primary-foreground text-sm rounded transition-colors"
                >
                  {executing === tool.name ? (
                    <>
                      <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                      Executing...
                    </>
                  ) : (
                    <>
                      <Play className="w-4 h-4" />
                      Execute Tool
                    </>
                  )}
                </button>

                {/* Results */}
                {results[tool.name] && (
                  <div className="mt-4 p-3 bg-muted rounded border">
                    <div className="flex items-center justify-between mb-2">
                      <h4 className="text-xs font-semibold text-muted-foreground">RESULT</h4>
                      <button
                        onClick={() => copyResult(tool.name)}
                        className="flex items-center gap-1 px-2 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-background rounded transition-colors"
                        title="Copy result to clipboard"
                      >
                        {copiedTool === tool.name ? (
                          <>
                            <Check className="w-3 h-3" />
                            Copied!
                          </>
                        ) : (
                          <>
                            <Copy className="w-3 h-3" />
                            Copy
                          </>
                        )}
                      </button>
                    </div>
                    <pre className="text-xs overflow-x-auto">
                      {JSON.stringify(results[tool.name], null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            )}
          </div>
            ))}
          </div>

          {tools.length === 0 && !error && (
            <div className="text-center py-8 text-muted-foreground">
              <Terminal className="w-12 h-12 mx-auto mb-2 opacity-50" />
              <p className="text-sm">Loading MCP tools...</p>
            </div>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
};

export default McpTools;