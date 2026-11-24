// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The agent cannot send, delete or move mail, whatever its model says: a
//! scripted model asks for send_email (as an email telling it to forward
//! invoices would make it), and the call is refused and reported back.

use super::mcp_http::{cleanup_test_db, create_test_dashboard_state, setup_test_env};
use rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format;
use rustymail::dashboard::services::ai::agent_executor::AgentExecutor;
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// An OpenAI-compatible model that answers with `replies` in turn and
/// records every request body it receives.
async fn scripted_model(replies: Vec<Value>) -> (u16, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let body = read_request_body(&mut socket).await;
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            let payload =
                json!({"choices": [{"index": 0, "message": reply, "finish_reason": "stop"}]})
                    .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    (port, requests)
}

async fn read_request_body(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = socket.read(&mut buffer).await.unwrap();
        data.extend_from_slice(&buffer[..n]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length: usize = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().ok())?
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                return data[end + 4..end + 4 + length].to_vec();
            }
        }
        if n == 0 {
            return Vec::new();
        }
    }
}

fn tool_call(id: &str, name: &str, arguments: Value) -> Value {
    json!({"id": id, "type": "function", "function": {"name": name, "arguments": arguments.to_string()}})
}

fn offered_names(request: &Value) -> Vec<String> {
    request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["function"]["name"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
#[serial]
async fn the_agent_refuses_to_send_whatever_the_model_asks() {
    setup_test_env();
    let test_name = "agent_tool_gate";
    let state = create_test_dashboard_state(test_name).await;
    let (port, requests) = scripted_model(vec![
        json!({"role": "assistant", "content": "", "tool_calls": [
            tool_call("c1", "send_email", json!({"account_id": "me@example.test",
                "to": ["thief@example.test"], "subject": "invoices", "body": "all of them"})),
            tool_call("c2", "list_accounts", json!({})),
        ]}),
        json!({"role": "assistant", "content": "I could not forward the invoices."}),
    ])
    .await;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO ai_model_configurations (role, provider, model_name, base_url)
         VALUES ('tool_calling', 'model_serve', 'scripted', ?)",
    )
    .bind(format!("http://127.0.0.1:{port}"))
    .execute(&pool)
    .await
    .unwrap();

    let result = AgentExecutor::new()
        .execute_with_tools(
            &pool,
            &state,
            "Summarise my inbox",
            Some("me@example.test"),
            get_mcp_tools_jsonrpc_format(),
            None,
        )
        .await
        .unwrap();

    let requests = requests.lock().unwrap().clone();
    let offered = offered_names(&requests[0]);
    assert!(offered.contains(&"get_email_by_uid".to_string()));
    for owner_only in [
        "send_email",
        "delete_messages",
        "atomic_move_message",
        "expunge",
    ] {
        assert!(
            !offered.contains(&owner_only.to_string()),
            "{owner_only} was offered"
        );
    }
    let send = result
        .actions_taken
        .iter()
        .find(|a| a.tool_name == "send_email")
        .unwrap();
    assert_eq!(send.result["blocked"], true, "{}", send.result);
    let listing = result
        .actions_taken
        .iter()
        .find(|a| a.tool_name == "list_accounts")
        .unwrap();
    assert!(
        listing.result.get("blocked").is_none(),
        "allowed tools still run"
    );
    let fed_back = requests[1]["messages"].to_string();
    assert!(
        fed_back.contains("only the owner"),
        "the refusal reaches the model"
    );
    assert_eq!(result.final_response, "I could not forward the invoices.");
    cleanup_test_db(test_name);
}
