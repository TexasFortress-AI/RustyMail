// src/dashboard/services/ai/agent_executor.rs
// Agent executor for running sub-agents with iterative tool calling

use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use reqwest::Client;
use log::{debug, error, warn, info};
use sqlx::SqlitePool;
use crate::api::errors::ApiError;
use crate::dashboard::services::DashboardState;
use super::model_config::{get_model_config, ModelConfiguration};
use super::tool_converter::{mcp_to_ollama_tools, parse_ollama_tool_call};

/// Default maximum iterations to prevent infinite loops
/// Can be overridden via AGENT_MAX_ITERATIONS environment variable
const DEFAULT_MAX_ITERATIONS: usize = 1000;

/// Get max iterations from environment or use default
fn get_max_iterations() -> usize {
    std::env::var("AGENT_MAX_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_ITERATIONS)
}

/// Agent execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResult {
    pub success: bool,
    pub final_response: String,
    pub actions_taken: Vec<ActionLog>,
    pub iterations: usize,
    pub error: Option<String>,
}

/// Log of an action taken by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    pub tool_name: String,
    pub arguments: Value,
    pub result: Value,
}

/// Agent executor
pub struct AgentExecutor {
    http_client: Client,
}

impl AgentExecutor {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    /// Execute an instruction with access to tools
    /// Uses iterative tool calling until the task is complete
    pub async fn execute_with_tools(
        &self,
        pool: &SqlitePool,
        state: &DashboardState,
        instruction: &str,
        account_id: Option<&str>,
        tools: Vec<Value>,  // MCP tool definitions
    ) -> Result<AgentResult, ApiError> {
        info!("Executing instruction with {} tools available", tools.len());

        // Get tool-calling model configuration
        let config = get_model_config(pool, "tool_calling").await?;

        // Convert MCP tools to Ollama format
        let ollama_tools = mcp_to_ollama_tools(&tools);
        debug!("Converted {} MCP tools to Ollama format", ollama_tools.len());

        // Build instruction with account context if provided
        let full_instruction = if let Some(acc_id) = account_id {
            format!("Account: {}\n\nInstruction: {}\n\nIMPORTANT: When calling tools that require an account_id parameter, use '{}'.", acc_id, instruction, acc_id)
        } else {
            instruction.to_string()
        };

        // Initialize conversation with user instruction
        let mut messages = vec![
            json!({
                "role": "user",
                "content": full_instruction
            })
        ];

        let mut actions_taken = Vec::new();
        let mut iteration = 0;

        // Iterative tool calling loop
        loop {
            iteration += 1;

            let max_iterations = get_max_iterations();
            if iteration > max_iterations {
                warn!("Reached maximum iterations ({})", max_iterations);
                return Ok(AgentResult {
                    success: false,
                    final_response: "Task exceeded maximum iterations".to_string(),
                    actions_taken,
                    iterations: iteration - 1,
                    error: Some("Maximum iterations exceeded".to_string()),
                });
            }

            debug!("Iteration {}: Calling model with {} messages", iteration, messages.len());

            // Call the model with tools
            let response = self.call_model_with_tools(&config, &messages, &ollama_tools).await?;

            // Check if the model wants to call tools
            if let Some(tool_calls) = response.get("tool_calls") {
                debug!("Model requested {} tool calls", tool_calls.as_array().map(|a| a.len()).unwrap_or(0));

                // Add assistant message with tool calls to conversation
                messages.push(response.clone());

                // Execute each tool call
                let tool_calls_array = tool_calls.as_array().ok_or_else(|| {
                    ApiError::InternalError {
                        message: "tool_calls is not an array".to_string(),
                    }
                })?;

                for tool_call in tool_calls_array {
                    let (tool_name, arguments) = parse_ollama_tool_call(tool_call).ok_or_else(|| {
                        ApiError::InternalError {
                            message: "Failed to parse tool call".to_string(),
                        }
                    })?;

                    debug!("Executing tool: {} with args: {:?}", tool_name, arguments);

                    // Execute the tool using existing handlers
                    let result = crate::dashboard::api::handlers::execute_mcp_tool_inner(
                        state,
                        &tool_name,
                        arguments.clone(),
                    ).await;

                    // Log the action
                    actions_taken.push(ActionLog {
                        tool_name: tool_name.clone(),
                        arguments: arguments.clone(),
                        result: result.clone(),
                    });

                    // Add tool response to conversation
                    let tool_call_id = tool_call.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
                    }));
                }
            } else {
                // No tool calls - model has finished
                let final_response = response.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("Task completed");

                info!("Agent completed task in {} iterations with {} actions", iteration, actions_taken.len());

                return Ok(AgentResult {
                    success: true,
                    final_response: final_response.to_string(),
                    actions_taken,
                    iterations: iteration,
                    error: None,
                });
            }
        }
    }

    /// Call the model with tools available
    async fn call_model_with_tools(
        &self,
        config: &ModelConfiguration,
        messages: &[Value],
        tools: &[Value],
    ) -> Result<Value, ApiError> {
        match config.provider.as_str() {
            "ollama" => self.call_ollama_with_tools(config, messages, tools).await,
            provider => {
                error!("Unsupported provider for tool calling: {}", provider);
                Err(ApiError::BadRequest {
                    message: format!("Unsupported tool-calling provider: {}", provider),
                })
            }
        }
    }

    /// Call Ollama with tool calling enabled
    async fn call_ollama_with_tools(
        &self,
        config: &ModelConfiguration,
        messages: &[Value],
        tools: &[Value],
    ) -> Result<Value, ApiError> {
        let base_url = config.base_url.as_deref()
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OLLAMA_BASE_URL").ok())
            .ok_or_else(|| ApiError::BadRequest {
                message: "OLLAMA_BASE_URL environment variable or base_url config must be set".to_string(),
            })?;
        let base_url = base_url.as_str();
        let url = format!("{}/v1/chat/completions", base_url);

        debug!("Calling Ollama at {} with model {} and {} tools", url, config.model_name, tools.len());

        let request_body = json!({
            "model": config.model_name,
            "messages": messages,
            "tools": tools,
            "stream": false,
        });

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(300))  // 5 minutes for large tool contexts
            .send()
            .await
            .map_err(|e| {
                error!("Failed to call Ollama API: {}", e);
                ApiError::ServiceUnavailable {
                    service: format!("Ollama API: {}", e),
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error>".to_string());
            error!("Ollama API returned error {}: {}", status, error_body);
            return Err(ApiError::ServiceUnavailable {
                service: format!("Ollama returned status {}: {}", status, error_body),
            });
        }

        let response_body: Value = response.json().await
            .map_err(|e| {
                error!("Failed to parse Ollama response: {}", e);
                ApiError::InternalError {
                    message: format!("Failed to parse response: {}", e),
                }
            })?;

        // Extract the assistant message
        let message = response_body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .ok_or_else(|| {
                error!("Ollama response missing expected message field");
                ApiError::InternalError {
                    message: "Invalid response format from Ollama".to_string(),
                }
            })?;

        Ok(message.clone())
    }
}

impl Default for AgentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_log_serialization() {
        let action = ActionLog {
            tool_name: "send_email".to_string(),
            arguments: json!({"to": "test@example.com"}),
            result: json!({"success": true}),
        };

        let serialized = serde_json::to_string(&action).unwrap();
        assert!(serialized.contains("send_email"));
        assert!(serialized.contains("test@example.com"));
    }

    #[test]
    fn test_agent_result_structure() {
        let result = AgentResult {
            success: true,
            final_response: "Email sent successfully".to_string(),
            actions_taken: vec![],
            iterations: 2,
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.iterations, 2);
        assert!(result.error.is_none());
    }
}
