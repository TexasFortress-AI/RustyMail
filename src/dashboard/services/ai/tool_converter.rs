// src/dashboard/services/ai/tool_converter.rs
// Convert MCP tool definitions to Ollama/OpenAI tool format

use serde_json::{json, Value};
use log::{debug, warn};

/// Convert MCP tools to Ollama/OpenAI tool format
///
/// MCP tools have format:
/// ```json
/// {
///   "name": "tool_name",
///   "description": "Description",
///   "inputSchema": {
///     "type": "object",
///     "properties": {...},
///     "required": [...]
///   }
/// }
/// ```
///
/// Ollama/OpenAI tools have format:
/// ```json
/// {
///   "type": "function",
///   "function": {
///     "name": "function_name",
///     "description": "Description",
///     "parameters": {
///       "type": "object",
///       "properties": {...},
///       "required": [...]
///     }
///   }
/// }
/// ```
pub fn mcp_to_ollama_tools(mcp_tools: &[Value]) -> Vec<Value> {
    debug!("Converting {} MCP tools to Ollama format", mcp_tools.len());

    mcp_tools
        .iter()
        .filter_map(|tool| mcp_tool_to_ollama(tool))
        .collect()
}

/// Convert a single MCP tool to Ollama/OpenAI format
fn mcp_tool_to_ollama(mcp_tool: &Value) -> Option<Value> {
    let name = mcp_tool.get("name")?.as_str()?;
    let description = mcp_tool.get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("");

    // Get inputSchema (MCP format) and convert to parameters (OpenAI format)
    let input_schema = mcp_tool.get("inputSchema")?;

    // Validate that inputSchema is an object with "type": "object"
    if input_schema.get("type")?.as_str()? != "object" {
        warn!("Tool {} has non-object inputSchema, skipping", name);
        return None;
    }

    // Build the parameters object (same as inputSchema for JSON Schema)
    let parameters = json!({
        "type": "object",
        "properties": input_schema.get("properties").unwrap_or(&json!({})).clone(),
        "required": input_schema.get("required").unwrap_or(&json!([])).clone(),
    });

    // Build the Ollama/OpenAI tool format
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters
        }
    }))
}

/// Convert Ollama tool call to MCP tool call format
///
/// Ollama tool calls have format:
/// ```json
/// {
///   "id": "call_abc123",
///   "type": "function",
///   "function": {
///     "name": "function_name",
///     "arguments": "{\"param\": \"value\"}"  // JSON string
///   }
/// }
/// ```
///
/// We need to parse the arguments string and return the function name and parsed arguments
pub fn parse_ollama_tool_call(tool_call: &Value) -> Option<(String, Value)> {
    let function = tool_call.get("function")?;
    let name = function.get("name")?.as_str()?.to_string();

    // Arguments come as a JSON string, need to parse it
    let arguments_str = function.get("arguments")?.as_str()?;
    let arguments = match serde_json::from_str::<Value>(arguments_str) {
        Ok(args) => args,
        Err(e) => {
            warn!("Failed to parse tool call arguments for {}: {}", name, e);
            json!({})
        }
    };

    Some((name, arguments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_to_ollama_conversion() {
        let mcp_tools = vec![
            json!({
                "name": "send_email",
                "description": "Send an email",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "to": {
                            "type": "string",
                            "description": "Recipient email"
                        },
                        "subject": {
                            "type": "string",
                            "description": "Email subject"
                        }
                    },
                    "required": ["to", "subject"]
                }
            })
        ];

        let ollama_tools = mcp_to_ollama_tools(&mcp_tools);

        assert_eq!(ollama_tools.len(), 1);
        let tool = &ollama_tools[0];

        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], "send_email");
        assert_eq!(tool["function"]["description"], "Send an email");
        assert_eq!(tool["function"]["parameters"]["type"], "object");
        assert!(tool["function"]["parameters"]["properties"]["to"].is_object());
        assert_eq!(tool["function"]["parameters"]["required"][0], "to");
    }

    #[test]
    fn test_parse_ollama_tool_call() {
        let tool_call = json!({
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "send_email",
                "arguments": "{\"to\":\"user@example.com\",\"subject\":\"Hello\"}"
            }
        });

        let (name, arguments) = parse_ollama_tool_call(&tool_call).unwrap();

        assert_eq!(name, "send_email");
        assert_eq!(arguments["to"], "user@example.com");
        assert_eq!(arguments["subject"], "Hello");
    }

    #[test]
    fn test_invalid_mcp_tool() {
        let mcp_tools = vec![
            json!({
                "name": "bad_tool",
                "inputSchema": {
                    "type": "string"  // Invalid: should be "object"
                }
            })
        ];

        let ollama_tools = mcp_to_ollama_tools(&mcp_tools);
        assert_eq!(ollama_tools.len(), 0);  // Should be filtered out
    }
}
