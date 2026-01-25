// src/dashboard/services/ai/email_drafter.rs
// Email drafting service using configured AI models

use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use reqwest::Client;
use log::{debug, error, warn, info};
use sqlx::SqlitePool;
use crate::api::errors::ApiError;
use super::model_config::{get_model_config, ModelConfiguration};
use super::sampler_config::{get_sampler_config, SamplerConfig};

/// Providers that support email drafting
/// These have been tested with their respective API formats
pub const DRAFTING_PROVIDERS: &[&str] = &["ollama", "openai", "llamacpp", "lmstudio"];

/// Email drafter service
pub struct EmailDrafter {
    http_client: Client,
}

/// Draft email request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftEmailRequest {
    pub to: String,
    pub subject: String,
    pub context: String,
}

/// Draft reply request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftReplyRequest {
    pub original_from: String,
    pub original_subject: String,
    pub original_body: String,
    pub instruction: Option<String>,
}

impl EmailDrafter {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    /// Draft a reply to an existing email
    pub async fn draft_reply(
        &self,
        pool: &SqlitePool,
        request: DraftReplyRequest,
    ) -> Result<String, ApiError> {
        debug!("Drafting reply to email from {}", request.original_from);

        // Get drafting model configuration
        let config = get_model_config(pool, "drafting").await?;

        // Fetch sampler config from database for this provider/model
        let sampler_config = get_sampler_config(pool, &config.provider, &config.model_name).await
            .map_err(|e| {
                warn!("Failed to get sampler config for drafting, using defaults: {:?}", e);
            }).ok();

        if sampler_config.is_some() {
            info!("Loaded sampler config from database for drafting {}/{}", config.provider, config.model_name);
        }

        // Build the prompt for the AI
        let prompt = self.build_reply_prompt(&request);

        // Generate the draft using the configured model
        self.generate_with_model(&config, &prompt, sampler_config.as_ref()).await
    }

    /// Draft a new email from scratch
    pub async fn draft_email(
        &self,
        pool: &SqlitePool,
        request: DraftEmailRequest,
    ) -> Result<String, ApiError> {
        debug!("Drafting new email to {} with subject: {}", request.to, request.subject);

        // Get drafting model configuration
        let config = get_model_config(pool, "drafting").await?;

        // Fetch sampler config from database for this provider/model
        let sampler_config = get_sampler_config(pool, &config.provider, &config.model_name).await
            .map_err(|e| {
                warn!("Failed to get sampler config for drafting, using defaults: {:?}", e);
            }).ok();

        if sampler_config.is_some() {
            info!("Loaded sampler config from database for drafting {}/{}", config.provider, config.model_name);
        }

        // Build the prompt for the AI
        let prompt = self.build_email_prompt(&request);

        // Generate the draft using the configured model
        self.generate_with_model(&config, &prompt, sampler_config.as_ref()).await
    }

    /// Build prompt for replying to an email
    fn build_reply_prompt(&self, request: &DraftReplyRequest) -> String {
        let instruction = request.instruction.as_deref().unwrap_or("write a professional reply");

        format!(
            r#"You are drafting a reply to an email. Please write ONLY the body of the reply email, without any greeting or signature (those will be added automatically).

Original Email:
From: {}
Subject: {}

{}

Instructions: {}

Draft reply body:"#,
            request.original_from,
            request.original_subject,
            request.original_body,
            instruction
        )
    }

    /// Build prompt for drafting a new email
    fn build_email_prompt(&self, request: &DraftEmailRequest) -> String {
        format!(
            r#"You are drafting a new email. Please write ONLY the body of the email, without any greeting or signature (those will be added automatically).

To: {}
Subject: {}

Context/Instructions: {}

Draft email body:"#,
            request.to,
            request.subject,
            request.context
        )
    }

    /// Generate text using the configured model
    async fn generate_with_model(
        &self,
        config: &ModelConfiguration,
        prompt: &str,
        sampler_config: Option<&SamplerConfig>,
    ) -> Result<String, ApiError> {
        match config.provider.as_str() {
            "ollama" => self.generate_with_ollama(config, prompt, sampler_config).await,
            "openai" => self.generate_with_openai(config, prompt, sampler_config).await,
            "llamacpp" => self.generate_with_openai_compatible(config, prompt, sampler_config, "LLAMACPP_BASE_URL").await,
            "lmstudio" => self.generate_with_openai_compatible(config, prompt, sampler_config, "LMSTUDIO_BASE_URL").await,
            provider => {
                error!("Unsupported provider for drafting: {}. Supported: {:?}", provider, DRAFTING_PROVIDERS);
                Err(ApiError::BadRequest {
                    message: format!("Unsupported drafting provider: '{}'. Supported: {}", provider, DRAFTING_PROVIDERS.join(", ")),
                })
            }
        }
    }

    /// Generate text using Ollama
    async fn generate_with_ollama(
        &self,
        config: &ModelConfiguration,
        prompt: &str,
        sampler_config: Option<&SamplerConfig>,
    ) -> Result<String, ApiError> {
        let base_url = config.base_url.as_deref()
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OLLAMA_BASE_URL").ok())
            .ok_or_else(|| ApiError::BadRequest {
                message: "OLLAMA_BASE_URL environment variable or base_url config must be set".to_string(),
            })?;

        // Use native /api/chat for full sampler control
        let url = format!("{}/api/chat", base_url);

        debug!("Calling Ollama native API at {} with model {}", url, config.model_name);

        // Build request with sampler config from database if available
        let request_body = if let Some(cfg) = sampler_config {
            info!("Applying sampler config to drafting: temp={:?}, top_p={:?}, min_p={:?}, num_ctx={:?}",
                  cfg.temperature, cfg.top_p, cfg.min_p, cfg.num_ctx);
            json!({
                "model": config.model_name,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "stream": false,
                "options": {
                    "temperature": cfg.effective_temperature(),
                    "top_p": cfg.effective_top_p(),
                    "top_k": cfg.top_k,
                    "min_p": cfg.effective_min_p(),
                    "repeat_penalty": cfg.effective_repeat_penalty(),
                    "num_ctx": cfg.effective_num_ctx(),
                    "think": cfg.effective_think_mode(),
                }
            })
        } else {
            json!({
                "model": config.model_name,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "stream": false,
                "options": {
                    "temperature": 0.7,
                }
            })
        };

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(120)) // Longer timeout for generation
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

        // Native API returns: {"message": {"role": "assistant", "content": "..."}}
        let content = response_body
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                error!("Ollama native API response missing expected content field: {:?}", response_body);
                ApiError::InternalError {
                    message: "Invalid response format from Ollama native API".to_string(),
                }
            })?;

        debug!("Successfully generated draft with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Generate text using OpenAI
    async fn generate_with_openai(
        &self,
        config: &ModelConfiguration,
        prompt: &str,
        sampler_config: Option<&SamplerConfig>,
    ) -> Result<String, ApiError> {
        let base_url = config.base_url.as_deref()
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .ok_or_else(|| ApiError::BadRequest {
                message: "OPENAI_BASE_URL environment variable or base_url config must be set".to_string(),
            })?;
        let url = format!("{}/chat/completions", base_url);

        let api_key = config.api_key.as_deref().ok_or_else(|| {
            ApiError::BadRequest {
                message: "OpenAI API key not configured".to_string(),
            }
        })?;

        debug!("Calling OpenAI API with model {}", config.model_name);

        // Apply sampler config if available (OpenAI supports temperature, top_p, max_tokens)
        let temperature = sampler_config.map(|c| c.effective_temperature()).unwrap_or(0.7);
        let top_p = sampler_config.and_then(|c| c.top_p);

        let mut request_body = json!({
            "model": config.model_name,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": temperature,
        });

        // Add optional parameters if present in sampler config
        if let Some(p) = top_p {
            request_body["top_p"] = json!(p);
        }
        if let Some(cfg) = sampler_config {
            if let Some(max) = cfg.max_tokens {
                request_body["max_tokens"] = json!(max);
            }
        }

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to call OpenAI API: {}", e);
                ApiError::ServiceUnavailable {
                    service: format!("OpenAI API: {}", e),
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error>".to_string());
            error!("OpenAI API returned error {}: {}", status, error_body);
            return Err(ApiError::ServiceUnavailable {
                service: format!("OpenAI returned status {}: {}", status, error_body),
            });
        }

        let response_body: Value = response.json().await
            .map_err(|e| {
                error!("Failed to parse OpenAI response: {}", e);
                ApiError::InternalError {
                    message: format!("Failed to parse response: {}", e),
                }
            })?;

        // Extract the generated text
        let content = response_body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                error!("OpenAI response missing expected content field");
                ApiError::InternalError {
                    message: "Invalid response format from OpenAI".to_string(),
                }
            })?;

        debug!("Successfully generated draft with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Generate text using OpenAI-compatible API (llama.cpp, LM Studio)
    async fn generate_with_openai_compatible(
        &self,
        config: &ModelConfiguration,
        prompt: &str,
        sampler_config: Option<&SamplerConfig>,
        env_var: &str,
    ) -> Result<String, ApiError> {
        let base_url = config.base_url.as_deref()
            .map(|s| s.to_string())
            .or_else(|| std::env::var(env_var).ok())
            .ok_or_else(|| ApiError::BadRequest {
                message: format!("{} environment variable or base_url config must be set", env_var),
            })?;

        let url = format!("{}/v1/chat/completions", base_url);

        debug!("Calling OpenAI-compatible API at {} with model {}", url, config.model_name);

        // Apply sampler config if available
        let temperature = sampler_config.map(|c| c.effective_temperature()).unwrap_or(0.7);
        let top_p = sampler_config.and_then(|c| c.top_p);
        let min_p = sampler_config.and_then(|c| c.min_p);

        let mut request_body = json!({
            "model": config.model_name,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": temperature,
            "stream": false,
        });

        // Add optional parameters if present
        if let Some(p) = top_p {
            request_body["top_p"] = json!(p);
        }
        if let Some(p) = min_p {
            request_body["min_p"] = json!(p);
        }
        if let Some(cfg) = sampler_config {
            if let Some(max) = cfg.max_tokens {
                request_body["max_tokens"] = json!(max);
            }
            if let Some(rp) = cfg.repeat_penalty {
                request_body["repeat_penalty"] = json!(rp);
            }
        }

        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(120))  // Longer timeout for local models
            .send()
            .await
            .map_err(|e| {
                error!("Failed to call OpenAI-compatible API: {}", e);
                ApiError::ServiceUnavailable {
                    service: format!("OpenAI-compatible API: {}", e),
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error>".to_string());
            error!("OpenAI-compatible API returned error {}: {}", status, error_body);
            return Err(ApiError::ServiceUnavailable {
                service: format!("OpenAI-compatible API returned status {}: {}", status, error_body),
            });
        }

        let response_body: Value = response.json().await
            .map_err(|e| {
                error!("Failed to parse OpenAI-compatible response: {}", e);
                ApiError::InternalError {
                    message: format!("Failed to parse response: {}", e),
                }
            })?;

        // OpenAI-compatible format: {"choices": [{"message": {"content": "..."}}]}
        let content = response_body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                error!("OpenAI-compatible API response missing expected content field: {:?}", response_body);
                ApiError::InternalError {
                    message: "Invalid response format from OpenAI-compatible API".to_string(),
                }
            })?;

        debug!("Successfully generated draft with {} characters", content.len());
        Ok(content.to_string())
    }
}

impl Default for EmailDrafter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_reply_prompt() {
        let drafter = EmailDrafter::new();
        let request = DraftReplyRequest {
            original_from: "john@example.com".to_string(),
            original_subject: "Meeting tomorrow".to_string(),
            original_body: "Can we meet at 2pm?".to_string(),
            instruction: Some("confirm and suggest 3pm instead".to_string()),
        };

        let prompt = drafter.build_reply_prompt(&request);

        assert!(prompt.contains("john@example.com"));
        assert!(prompt.contains("Meeting tomorrow"));
        assert!(prompt.contains("Can we meet at 2pm?"));
        assert!(prompt.contains("confirm and suggest 3pm instead"));
    }

    #[test]
    fn test_build_email_prompt() {
        let drafter = EmailDrafter::new();
        let request = DraftEmailRequest {
            to: "jane@example.com".to_string(),
            subject: "Project Update".to_string(),
            context: "Let her know the project is on track".to_string(),
        };

        let prompt = drafter.build_email_prompt(&request);

        assert!(prompt.contains("jane@example.com"));
        assert!(prompt.contains("Project Update"));
        assert!(prompt.contains("Let her know the project is on track"));
    }
}
