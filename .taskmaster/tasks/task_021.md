# Task ID: 21

**Title:** Disable thinking mode in Qwen3 model by setting enable_thinking=False in Ollama provider configuration

**Status:** done

**Dependencies:** 2 ✓, 4 ✓

**Priority:** medium

**Description:** Modify Ollama provider to support additional_config parameters and update Qwen3 model configuration to disable thinking mode for faster response times.

**Details:**

Update the Ollama provider implementation to read and apply additional_config parameters from the ai_model_configurations table: 1) Modify OllamaChatRequest struct in src/dashboard/services/ai/provider/ollama.rs to include optional additional parameters field, 2) Update OllamaAdapter::generate_response() method to fetch model configuration using get_model_config() and parse additional_config JSON to extract provider-specific parameters, 3) Add logic to merge additional_config parameters into the Ollama API request payload, 4) Use set_model_config() to update the Qwen3 model configuration with additional_config JSON: {"enable_thinking": false}, 5) Test that the parameter is properly passed to Ollama API and that thinking blocks are no longer generated in responses. The additional_config field should be parsed as JSON and merged into the request body sent to Ollama's /v1/chat/completions endpoint.

**Test Strategy:**

Test by: 1) Verifying that the Ollama provider properly reads additional_config from database and parses JSON parameters, 2) Confirming that enable_thinking=false parameter is included in API requests to Ollama for Qwen3 model, 3) Testing that Qwen3 responses no longer contain <think> blocks and show improved response speed, 4) Verifying that other models without this configuration continue to work normally, 5) Testing configuration updates through MCP tools to ensure the additional_config field can be modified and persisted correctly.
