# Task ID: 37

**Title:** Add LM Studio provider adapter

**Status:** done

**Dependencies:** 34 ✓

**Priority:** medium

**Description:** Create a new LM Studio adapter similar to the llama.cpp adapter since LM Studio uses an OpenAI-compatible API but may have specific quirks. Support the same sampler settings as llama.cpp (min_p, top_no, etc.).

**Details:**

Create src/dashboard/services/ai/providers/lmstudio.rs with the following implementation:

1. Define LMStudioProvider struct that implements the AI provider trait:
   - base_url: String (from LMSTUDIO_BASE_URL env var, default: "http://localhost:1234")
   - api_key: Option<String> (LM Studio typically doesn't require auth)
   - client: reqwest::Client

2. Implement provider methods:
   - new() - Initialize with base URL from env or default
   - complete() - Send completion requests to /v1/chat/completions endpoint
   - complete_with_tools() - Send tool-enabled requests (if LM Studio supports it)
   - list_models() - Query /v1/models endpoint

3. Support LM Studio-specific sampler parameters in requests:
   - temperature, top_p, top_k (standard OpenAI params)
   - min_p (minimum probability threshold)
   - top_a (top-a sampling)
   - typical_p (typical sampling)
   - tfs_z (tail-free sampling)
   - repeat_penalty
   - repeat_last_n
   - penalize_nl
   - presence_penalty
   - frequency_penalty
   - mirostat, mirostat_tau, mirostat_eta
   - seed
   - stop sequences

4. Handle LM Studio response format quirks:
   - Parse streaming responses if different from OpenAI
   - Handle any non-standard error formats
   - Support both streaming and non-streaming modes

5. Add provider registration:
   - Update provider factory/registry to include "lmstudio" as a provider option
   - Ensure SamplerConfigService can apply LM Studio-specific samplers

6. Environment variable support:
   - LMSTUDIO_BASE_URL (default: http://localhost:1234)
   - LMSTUDIO_API_KEY (optional, for future compatibility)

7. Error handling:
   - Connection errors when LM Studio isn't running
   - Model not loaded errors
   - Invalid sampler parameter combinations
   - Timeout handling for slow local models

Example implementation structure:
```rust
use crate::dashboard::services::ai::{AIProvider, CompletionRequest, CompletionResponse};

pub struct LMStudioProvider {
    base_url: String,
    client: reqwest::Client,
}

impl LMStudioProvider {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let base_url = std::env::var("LMSTUDIO_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:1234".to_string());
        
        Ok(Self {
            base_url,
            client: reqwest::Client::new(),
        })
    }
    
    fn apply_sampler_config(&self, request: &mut serde_json::Value, config: &SamplerConfiguration) {
        // Apply LM Studio specific samplers
        if let Some(min_p) = config.min_p {
            request["min_p"] = json!(min_p);
        }
        // ... other samplers
    }
}

impl AIProvider for LMStudioProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, Box<dyn std::error::Error>> {
        // Implementation
    }
}
```

**Test Strategy:**

1. Unit tests for LMStudioProvider:
   - Test initialization with default and custom LMSTUDIO_BASE_URL
   - Mock HTTP responses for completion requests
   - Test all sampler parameters are correctly included in requests
   - Test error handling for connection failures
   - Test streaming vs non-streaming response parsing

2. Integration tests (requires LM Studio running):
   - Test actual completion requests with a loaded model
   - Verify sampler parameters affect output (e.g., temperature 0 vs 1)
   - Test model listing endpoint
   - Test timeout handling with large prompts
   - Test special characters and Unicode handling

3. Manual testing checklist:
   - Install LM Studio on Windows/macOS
   - Load a model (e.g., Llama 3.3)
   - Configure RustyMail to use lmstudio provider
   - Test email drafting with various sampler settings
   - Verify min_p, top_a, and other LM Studio-specific samplers work
   - Test with different model architectures (Llama, Mistral, Qwen)
   - Test error messages when LM Studio isn't running
   - Verify performance with local models

4. Compatibility testing:
   - Test with latest LM Studio version
   - Verify OpenAI compatibility layer works as expected
   - Test any LM Studio-specific extensions or deviations
   - Document any limitations or unsupported features
