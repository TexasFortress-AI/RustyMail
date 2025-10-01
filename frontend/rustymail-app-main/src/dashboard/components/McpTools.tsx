import React, { useEffect, useState } from 'react';
import { ChevronDown, ChevronRight, Play, Terminal, Code } from 'lucide-react';

interface McpTool {
  name: string;
  description: string;
  parameters: { [key: string]: string };
}

const McpTools: React.FC = () => {
  const [tools, setTools] = useState<McpTool[]>([]);
  const [expandedTool, setExpandedTool] = useState<string | null>(null);
  const [executing, setExecuting] = useState<string | null>(null);
  const [results, setResults] = useState<{ [key: string]: any }>({});
  const [parameters, setParameters] = useState<{ [key: string]: { [key: string]: string } }>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchTools();
  }, []);

  const fetchTools = async () => {
    try {
      const response = await fetch('http://localhost:9437/api/dashboard/mcp/tools', {
        headers: {
          'X-API-Key': 'test-rustymail-key-2024'
        }
      });

      if (!response.ok) {
        throw new Error(`Failed to fetch tools: ${response.statusText}`);
      }

      const data = await response.json();
      setTools(data.tools || []);

      // Initialize parameters state for all tools
      const initialParams: { [key: string]: { [key: string]: string } } = {};
      data.tools?.forEach((tool: McpTool) => {
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
      const response = await fetch('http://localhost:9437/api/dashboard/mcp/execute', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-API-Key': 'test-rustymail-key-2024'
        },
        body: JSON.stringify({
          tool: toolName,
          parameters: parameters[toolName] || {}
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
    setExpandedTool(expandedTool === toolName ? null : toolName);
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

  return (
    <div className="bg-gray-800 rounded-lg p-4 h-full flex flex-col">
      <div className="flex items-center gap-2 mb-4 flex-shrink-0">
        <Terminal className="w-5 h-5 text-blue-400" />
        <h3 className="text-lg font-semibold text-white">MCP Email Tools</h3>
        <span className="text-xs text-gray-400 ml-auto">
          {tools.length} tools available
        </span>
      </div>

      {error && (
        <div className="bg-red-500/10 border border-red-500/50 rounded p-3 mb-4 flex-shrink-0">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      <div className="space-y-2 flex-1 overflow-y-auto">
        {tools.map(tool => (
          <div key={tool.name} className="border border-gray-700 rounded overflow-hidden">
            {/* Tool Header */}
            <button
              onClick={() => toggleTool(tool.name)}
              className="w-full flex items-center gap-2 p-3 bg-gray-750 hover:bg-gray-700 transition-colors"
            >
              {expandedTool === tool.name ?
                <ChevronDown className="w-4 h-4 text-gray-400" /> :
                <ChevronRight className="w-4 h-4 text-gray-400" />
              }
              <Code className="w-4 h-4 text-blue-400" />
              <span className="text-white font-mono text-sm">{tool.name}</span>
              <span className="text-gray-400 text-xs ml-auto">{tool.description}</span>
            </button>

            {/* Tool Body */}
            {expandedTool === tool.name && (
              <div className="p-4 bg-gray-900/50 border-t border-gray-700">
                {/* Parameters */}
                {Object.keys(tool.parameters).length > 0 && (
                  <div className="mb-4">
                    <h4 className="text-xs font-semibold text-gray-400 mb-2">PARAMETERS</h4>
                    <div className="space-y-2">
                      {Object.entries(tool.parameters).map(([paramName, paramDesc]) => (
                        <div key={paramName}>
                          <label className="block text-xs text-gray-400 mb-1">
                            {paramName}: <span className="text-gray-500">{paramDesc}</span>
                          </label>
                          <input
                            type="text"
                            value={parameters[tool.name]?.[paramName] || ''}
                            onChange={(e) => updateParameter(tool.name, paramName, e.target.value)}
                            className="w-full px-2 py-1 bg-gray-800 border border-gray-700 rounded text-white text-sm focus:outline-none focus:border-blue-500"
                            placeholder={`Enter ${paramName}`}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Execute Button */}
                <button
                  onClick={() => executeTool(tool.name)}
                  disabled={executing === tool.name}
                  className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 disabled:opacity-50 text-white text-sm rounded transition-colors"
                >
                  {executing === tool.name ? (
                    <>
                      <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
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
                  <div className="mt-4 p-3 bg-gray-800 rounded border border-gray-700">
                    <h4 className="text-xs font-semibold text-gray-400 mb-2">RESULT</h4>
                    <pre className="text-xs text-gray-300 overflow-x-auto">
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
        <div className="text-center py-8 text-gray-500">
          <Terminal className="w-12 h-12 mx-auto mb-2 opacity-50" />
          <p className="text-sm">Loading MCP tools...</p>
        </div>
      )}
    </div>
  );
};

export default McpTools;