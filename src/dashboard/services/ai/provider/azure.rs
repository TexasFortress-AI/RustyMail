// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/dashboard/services/ai/providers/azure.rs
// Azure OpenAI uses a different URL structure and API version

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use log::{debug, warn, error};
use crate::dashboard::api::errors::ApiError;
use super::{AiProvider, AiChatMessage};
use crate::api::errors::ApiError as RestApiError;

const DEFAULT_AZURE_API_VERSION: &str = "2024-10-01";
const DEFAULT_AZURE_DEPLOYMENT: &str = "gpt-4";

// --- Azure OpenAI Specific Request/Response Structs (OpenAI-compatible) ---
#[derive(Serialize)]
struct AzureChatRequest {
    messages: Vec<AiChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct AzureChatResponse {
    choices: Vec<AzureChoice>,
}

#[derive(Deserialize)]
struct AzureChoice {
    message: AiChatMessage,
}

#[derive(Clone)]
pub struct AzureOpenAIAdapter {
    api_key: String,
    http_client: Client,
    endpoint: String,          // e.g., https://your-resource.openai.azure.com
    deployment_name: String,    // The deployment ID/name
    api_version: String,        // API version
}

impl AzureOpenAIAdapter {
    pub fn new(api_key: String, http_client: Client) -> Result<Self, RestApiError> {
        // Azure requires AZURE_OPENAI_ENDPOINT to be set
        let endpoint = std::env::var("AZURE_OPENAI_ENDPOINT")
            .map_err(|_| RestApiError::UnprocessableEntity {
                message: "AZURE_OPENAI_ENDPOINT environment variable is required for Azure OpenAI".to_string()
            })?;

        let deployment_name = std::env::var("AZURE_OPENAI_DEPLOYMENT")
            .unwrap_or_else(|_| DEFAULT_AZURE_DEPLOYMENT.to_string());

        let api_version = std::env::var("AZURE_OPENAI_API_VERSION")
            .unwrap_or_else(|_| DEFAULT_AZURE_API_VERSION.to_string());

        Ok(Self {
            api_key,
            http_client,
            endpoint,
            deployment_name,
            api_version,
        })
    }

    #[allow(dead_code)]
    pub fn with_deployment(mut self, deployment: String) -> Self {
        self.deployment_name = deployment;
        self
    }
}

#[async_trait]
impl AiProvider for AzureOpenAIAdapter {
    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        // Azure OpenAI doesn't have a models list endpoint in the same way
        // Return the configured deployment name as the only "model"
        debug!("Returning configured Azure OpenAI deployment as available model");
        Ok(vec![self.deployment_name.clone()])
    }

    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        // Azure URL format: https://{resource-name}.openai.azure.com/openai/deployments/{deployment-id}/chat/completions?api-version={version}
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.endpoint.trim_end_matches('/'),
            self.deployment_name,
            self.api_version
        );

        let request_payload = AzureChatRequest {
            messages: messages.to_vec(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        debug!("Sending request to Azure OpenAI API: deployment={}, messages_count={}, url={}",
               self.deployment_name, request_payload.messages.len(), url);

        let response = self.http_client
            .post(&url)
            .header("api-key", &self.api_key)
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable { service: format!("Azure OpenAI: {}", e) })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!("Azure OpenAI API request failed with status {}: {}", status, error_body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("Azure OpenAI API returned error status {}: {}", status, error_body)
            });
        }

        let response_body = response
            .json::<AzureChatResponse>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity { message: format!("Failed to deserialize Azure OpenAI response: {}", e) })?;

        if let Some(choice) = response_body.choices.first() {
            debug!("Received response from Azure OpenAI API.");
            Ok(choice.message.content.clone())
        } else {
            warn!("Azure OpenAI API response did not contain any choices.");
            Err(RestApiError::UnprocessableEntity { message: "Azure OpenAI response was empty or missing choices".to_string() })
        }
    }
}
