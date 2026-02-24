# Task ID: 40

**Title:** Extend agent_executor tool-calling to support all configured providers

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Modify agent_executor.rs to support tool calling for all AI providers (OpenAI, Anthropic, etc.) beyond just Ollama, or restrict the UI to only show supported providers to prevent runtime errors.

**Details:**

Fix the limitation in src/dashboard/services/ai/agent_executor.rs line 189 where only 'ollama' provider is handled for tool calling:

1. **Analyze current implementation**:
   - Review the existing execute_with_tools() method that currently only supports Ollama
   - Identify the tool-calling format differences between providers (Ollama uses custom format, OpenAI uses function calling, Anthropic uses tools API)
   - Check how model configurations are loaded and which providers are configured

2. **Implement provider-specific tool calling adapters**:
   - Create a ToolCallingAdapter trait in agent_executor.rs with methods:
     ```rust
     trait ToolCallingAdapter {
         fn format_tools(&self, tools: &[Tool]) -> Value;
         fn parse_tool_calls(&self, response: &Value) -> Result<Vec<ToolCall>>;
         fn format_tool_results(&self, results: &[ToolResult]) -> Value;
     }
     ```
   - Implement OllamaToolAdapter (refactor existing code)
   - Implement OpenAIToolAdapter for OpenAI function calling format
   - Implement AnthropicToolAdapter for Anthropic tools API format
   - Add placeholder adapters for other providers that return "not supported" error

3. **Update AgentExecutor to use adapters**:
   - Modify execute_with_tools() to select appropriate adapter based on provider:
     ```rust
     let adapter: Box<dyn ToolCallingAdapter> = match provider.as_str() {
         "ollama" => Box::new(OllamaToolAdapter::new()),
         "openai" => Box::new(OpenAIToolAdapter::new()),
         "anthropic" => Box::new(AnthropicToolAdapter::new()),
         _ => return Err("Unsupported tool-calling provider")
     };
     ```
   - Use adapter methods to format tools, parse responses, and format results
   - Ensure error handling covers provider-specific edge cases

4. **Add provider capability detection**:
   - Create a supports_tool_calling() method in model configurations
   - Add a tool_calling_capable boolean field to provider configs
   - Update get_model_configurations tool to include this capability flag

5. **Update UI/API to respect capabilities**:
   - Modify the model selection endpoints to filter out non-tool-calling models when selecting tool-calling model
   - Add validation in set_tool_calling_model to reject unsupported providers
   - Return clear error messages when attempting to use unsupported providers

6. **Handle provider-specific nuances**:
   - OpenAI: Convert MCP tools to OpenAI function schema, handle function_call responses
   - Anthropic: Format tools according to Anthropic's tool use format, parse tool_use blocks
   - Add appropriate headers and API version parameters for each provider
   - Handle streaming vs non-streaming responses appropriately

**Test Strategy:**

Verify multi-provider tool calling support with comprehensive testing:

1. **Unit tests for tool adapters**:
   - Test OllamaToolAdapter formats tools correctly and parses Ollama-style responses
   - Test OpenAIToolAdapter converts MCP tools to OpenAI function schema
   - Test AnthropicToolAdapter formats according to Anthropic tool use spec
   - Verify error handling for malformed responses from each provider

2. **Integration tests for AgentExecutor**:
   - Mock responses from different providers and verify correct tool execution flow
   - Test provider selection logic with various model configurations
   - Verify fallback behavior for unsupported providers
   - Test error propagation when provider returns unexpected format

3. **End-to-end testing**:
   - Test process_email_instructions with Ollama model (existing functionality)
   - Test with OpenAI model configuration if available
   - Test with Anthropic model configuration if available
   - Verify UI correctly filters model selection based on capabilities

4. **Manual testing scenarios**:
   - Configure multiple providers in model_configurations
   - Attempt to set each as tool-calling model via API
   - Execute high-level tools and verify correct provider is used
   - Confirm error messages are clear when selecting unsupported provider

5. **Regression testing**:
   - Ensure existing Ollama tool calling still works correctly
   - Verify no breaking changes to high-level tool execution
   - Test that email processing workflows continue to function
