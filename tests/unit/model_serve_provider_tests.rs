// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! model_serve as a provider: RustyMail sends the model and messages only,
//! since model_serve's routes own every inference setting.

use actix_web::{web, App, HttpResponse, HttpServer};
use rustymail::dashboard::services::ai::local_only::ensure_local;
use rustymail::dashboard::services::ai::model_config::default_model_config;
use rustymail::dashboard::services::ai::provider::model_serve::{
    chat_request, model_ids, reply_text, server_owns_settings, ModelServeAdapter, PROVIDER,
};
use rustymail::dashboard::services::ai::provider::{AiChatMessage, AiProvider};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn hello() -> Vec<AiChatMessage> {
    vec![AiChatMessage {
        role: "user".into(),
        content: "hello".into(),
    }]
}

#[test]
fn requests_carry_no_inference_settings() {
    let body = chat_request("ms/gemma4", &hello());
    let keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 3, "{keys:?}");
    assert_eq!(body["model"], "ms/gemma4");
    assert_eq!(body["messages"][0]["content"], "hello");
    assert_eq!(body["stream"], false);
    assert!(server_owns_settings(PROVIDER));
    assert!(!server_owns_settings("ollama"));
    assert!(!server_owns_settings("lmstudio"));
}

#[test]
fn replies_and_model_lists_are_read() {
    let reply = json!({"choices": [{"message": {"role": "assistant", "content": "hi"}}]});
    assert_eq!(reply_text(&reply), Some("hi".to_string()));
    assert_eq!(reply_text(&json!({"choices": []})), None);
    let list = json!({"data": [{"id": "ms/gemma4"}, {"id": "ms/jev"}]});
    assert_eq!(model_ids(&list), vec!["ms/gemma4", "ms/jev"]);
    assert!(model_ids(&json!({})).is_empty());
}

#[test]
fn model_serve_is_local_and_preferred_for_unset_roles() {
    assert!(ensure_local(PROVIDER, Some("http://127.0.0.1:5846")).is_ok());
    assert!(ensure_local(PROVIDER, Some("https://models.example.com")).is_err());
    let config = default_model_config(
        "drafting",
        Some("ms/gemma4".into()),
        Some("gemma4:26b".into()),
    )
    .unwrap();
    assert_eq!(config.provider, PROVIDER);
    assert_eq!(config.model_name, "ms/gemma4");
}

#[actix_web::test]
async fn the_adapter_sends_only_model_and_messages() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let record = seen.clone();
    let server = HttpServer::new(move || {
        let record = record.clone();
        App::new()
            .route(
                "/v1/chat/completions",
                web::post().to(move |body: web::Json<Value>| {
                    let record = record.clone();
                    async move {
                        record.lock().unwrap().push(body.into_inner());
                        HttpResponse::Ok()
                            .json(json!({"choices": [{"message": {"content": "ok"}}]}))
                    }
                }),
            )
            .route(
                "/v1/models",
                web::get().to(|| async {
                    HttpResponse::Ok().json(json!({"data": [{"id": "ms/gemma4"}]}))
                }),
            )
    })
    .bind(("127.0.0.1", 0))
    .unwrap();
    let port = server.addrs()[0].port();
    let running = server.run();
    let handle = running.handle();
    actix_web::rt::spawn(running);

    let adapter = ModelServeAdapter::new(
        format!("http://127.0.0.1:{port}/"),
        reqwest::Client::new(),
        "ms/gemma4".into(),
    );
    assert_eq!(adapter.generate_response(&hello()).await.unwrap(), "ok");
    assert_eq!(
        adapter.get_available_models().await.unwrap(),
        vec!["ms/gemma4"]
    );
    let sent = seen.lock().unwrap().clone();
    assert_eq!(sent, vec![chat_request("ms/gemma4", &hello())]);
    handle.stop(true).await;
}

#[test]
fn models_that_cannot_chat_are_left_out_of_the_menu() {
    // model_serve may report Ollama's capabilities; a decision-only model
    // (the triage classifier) cannot chat.
    let list = json!({"data": [
        {"id": "ms/gemma4", "meta": {"model_serve": {"capabilities": ["completion", "tools"]}}},
        {"id": "ms/autotrust-jev-9b", "meta": {"model_serve": {"capabilities": ["decision"]}}},
        {"id": "ms/llama-swap-model"},
    ]});
    assert_eq!(model_ids(&list), vec!["ms/gemma4", "ms/llama-swap-model"]);
}
