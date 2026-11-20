// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! model_serve: a local model server whose routes own every inference
//! setting (context size, keep-alive, thinking, sampling). RustyMail sends
//! the model and the messages, and nothing else, so one loaded copy of a
//! model serves every caller. It speaks the OpenAI-compatible API.

use super::{
    can_chat, get_ai_generation_timeout, get_ai_request_timeout, AiChatMessage, AiProvider,
};
use crate::api::errors::ApiError as RestApiError;
use async_trait::async_trait;
use log::{debug, error};
use reqwest::Client;
use serde_json::{json, Value};

/// The provider's name in RustyMail's settings and database.
pub const PROVIDER: &str = "model_serve";
/// Base URL of the model_serve proxy, e.g. http://127.0.0.1:5846.
pub const BASE_URL_ENV: &str = "MODEL_SERVE_BASE_URL";
/// The model RustyMail uses on model_serve, e.g. ms/gemma4-26b-a4b-it-q4_k_m.
pub const MODEL_ENV: &str = "MODEL_SERVE_MODEL";

/// The configured model_serve model, when both its URL and model are set.
pub fn configured_model() -> Option<String> {
    std::env::var(BASE_URL_ENV).ok()?;
    std::env::var(MODEL_ENV)
        .ok()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
}

/// Whether the server, not RustyMail, decides this provider's inference
/// settings; RustyMail then sends none.
pub fn server_owns_settings(provider: &str) -> bool {
    provider == PROVIDER
}

/// A chat request with nothing but the model and the messages.
pub fn chat_request(model: &str, messages: &[AiChatMessage]) -> Value {
    json!({ "model": model, "messages": messages, "stream": false })
}

/// The reply text of an OpenAI-compatible chat completion.
pub fn reply_text(body: &Value) -> Option<String> {
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
}

/// Ids of the models that can chat in an OpenAI-compatible model list.
/// model_serve may report a model's capabilities under
/// meta.model_serve.capabilities; models without "completion" are left out.
pub fn model_ids(body: &Value) -> Vec<String> {
    body["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter(|model| can_chat(capabilities(model).as_deref()))
                .filter_map(|model| model["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn capabilities(model: &Value) -> Option<Vec<String>> {
    model["meta"]["model_serve"]["capabilities"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect()
        })
}

pub struct ModelServeAdapter {
    base_url: String,
    http_client: Client,
    model: String,
}

impl ModelServeAdapter {
    pub fn new(base_url: String, http_client: Client, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client,
            model,
        }
    }

    async fn get_json(&self, request: reqwest::RequestBuilder) -> Result<Value, RestApiError> {
        let response = request
            .send()
            .await
            .map_err(|e| RestApiError::ServiceUnavailable {
                service: format!("model_serve: {}", e),
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("model_serve returned {}: {}", status, body);
            return Err(RestApiError::ServiceUnavailable {
                service: format!("model_serve returned {}: {}", status, body),
            });
        }
        response
            .json::<Value>()
            .await
            .map_err(|e| RestApiError::UnprocessableEntity {
                message: format!("model_serve sent an unreadable reply: {}", e),
            })
    }
}

#[async_trait]
impl AiProvider for ModelServeAdapter {
    async fn generate_response(&self, messages: &[AiChatMessage]) -> Result<String, RestApiError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        debug!(
            "model_serve chat: model={}, messages={}",
            self.model,
            messages.len()
        );
        let request = self
            .http_client
            .post(&url)
            .json(&chat_request(&self.model, messages))
            .timeout(get_ai_generation_timeout());
        let body = self.get_json(request).await?;
        reply_text(&body).ok_or_else(|| RestApiError::UnprocessableEntity {
            message: "model_serve reply had no message content".to_string(),
        })
    }

    async fn get_available_models(&self) -> Result<Vec<String>, RestApiError> {
        let url = format!("{}/v1/models", self.base_url);
        let request = self.http_client.get(&url).timeout(get_ai_request_timeout());
        Ok(model_ids(&self.get_json(request).await?))
    }
}
