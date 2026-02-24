# Task ID: 41

**Title:** Extend email_drafter to support all configured providers

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Modify email_drafter.rs to use the provider adapter abstraction consistently so any configured provider (not just Ollama and OpenAI) works for email drafting, preventing runtime errors when users select other providers in the UI.

**Details:**

Fix the provider limitation in src/dashboard/services/ai/email_drafter.rs where only 'ollama' and 'openai' providers are hardcoded:

1. **Analyze current implementation**:
   - Review the existing draft_reply() and draft_email() methods that currently only support Ollama and OpenAI
   - Identify where provider-specific logic is hardcoded (likely in a match statement or if/else chain)
   - Check how the provider adapter abstraction is used in other services (e.g., agent_executor.rs after Task 40)

2. **Refactor to use provider adapter abstraction**:
   ```rust
   // Instead of hardcoding provider logic:
   match provider_name.as_str() {
       "ollama" => { /* Ollama-specific code */ }
       "openai" => { /* OpenAI-specific code */ }
       _ => return Err("Unsupported provider")
   }
   
   // Use the provider adapter pattern:
   let provider = self.provider_manager.get_provider(&provider_name)?;
   let response = provider.complete(CompletionRequest {
       model: model_name,
       messages: vec![
           Message::system("You are an email drafting assistant..."),
           Message::user(&prompt)
       ],
       temperature: Some(0.7),
       max_tokens: Some(1000),
       stream: false,
   }).await?;
   ```

3. **Update EmailDrafter struct**:
   - Add provider_manager: Arc<ProviderManager> field
   - Remove any provider-specific client fields (ollama_client, openai_client, etc.)
   - Update the constructor to accept ProviderManager

4. **Modify draft_reply() method**:
   - Remove provider-specific branching logic
   - Use provider_manager.get_provider() to get the appropriate adapter
   - Build a unified CompletionRequest that works with all providers
   - Handle the response uniformly regardless of provider

5. **Modify draft_email() method**:
   - Apply the same refactoring as draft_reply()
   - Ensure consistent error handling across all providers

6. **Update error handling**:
   - Replace provider-specific error types with a generic error type
   - Ensure meaningful error messages when a provider fails
   - Add logging for debugging provider issues

7. **Consider provider capabilities**:
   - Some providers may have different token limits or features
   - Use the provider adapter's capabilities to adjust request parameters
   - Gracefully degrade functionality if a provider doesn't support certain features

**Test Strategy:**

Verify multi-provider email drafting support with comprehensive testing:

1. **Unit tests for EmailDrafter**:
   - Mock ProviderManager to return different provider adapters
   - Test draft_reply() with mocked Ollama, OpenAI, Anthropic, LM Studio, and llama.cpp providers
   - Test draft_email() with all supported providers
   - Verify error handling when provider_manager.get_provider() fails
   - Test with providers that have different capabilities/limitations

2. **Integration tests with real providers**:
   - Set up test configurations for multiple providers in ai_model_configurations
   - Test actual email drafting with each configured provider
   - Verify response quality and format consistency across providers
   - Test error scenarios (invalid API keys, network failures, etc.)

3. **Manual testing through UI**:
   - Configure multiple providers in the system
   - Select each provider in the Email Assistant UI
   - Draft replies and new emails with each provider
   - Verify no runtime errors occur regardless of selected provider
   - Test switching between providers during a session

4. **Performance testing**:
   - Compare response times across different providers
   - Ensure no performance regression from the refactoring
   - Test concurrent drafting requests with different providers

5. **Regression testing**:
   - Ensure Ollama and OpenAI (previously working providers) still function correctly
   - Verify email quality hasn't degraded with the abstraction
   - Test edge cases like very long emails or special formatting
