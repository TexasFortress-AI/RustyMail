// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The chat assistant's model can read freely, but a send, delete or move it
//! asks for is never run: it is held for the owner with what it would touch.

use rustymail::dashboard::services::ai::chat_tool_gate::{handle_call, uids_in, ChatToolOutcome};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

fn set(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// A tool runner that records what it was asked to run.
fn recorder() -> (
    Arc<Mutex<Vec<String>>>,
    impl Fn(String, Value) -> std::future::Ready<Result<Value, String>>,
) {
    let ran = Arc::new(Mutex::new(Vec::new()));
    let log = ran.clone();
    let run = move |name: String, _args: Value| {
        log.lock().unwrap().push(name.clone());
        std::future::ready(Ok(if name == "get_email_by_uid" {
            json!({"success": true, "data": {"from_address": "boss@corp.test", "subject": "Q3 invoices"}})
        } else {
            json!({"success": true, "data": []})
        }))
    };
    (ran, run)
}

#[tokio::test]
async fn a_send_the_model_asks_for_waits_for_the_owner() {
    let (ran, run) = recorder();
    let offered = set(&["send_email", "list_accounts"]);
    let params = json!({"account_id": "me@x.test", "to": ["thief@evil.test"], "subject": "fwd", "body": "all"});
    let outcome = handle_call(
        "send_email",
        params.clone(),
        &offered,
        &set(&["send_email"]),
        run,
    )
    .await;
    let ChatToolOutcome::AwaitingOwner(action, line) = outcome else {
        panic!("not held: {outcome:?}");
    };
    assert_eq!(action.tool, "send_email");
    assert_eq!(action.parameters, params, "exactly what the owner will run");
    assert_eq!(action.variant, "low-level");
    assert!(line.contains("only the owner"));
    assert!(ran.lock().unwrap().is_empty(), "nothing was run");
}

#[tokio::test]
async fn a_delete_shows_the_server_s_description_of_each_message() {
    let (ran, run) = recorder();
    let params = json!({"account_id": "me@x.test", "folder": "INBOX", "uids": [7, 9]});
    let outcome = handle_call(
        "delete_messages",
        params,
        &set(&["delete_messages"]),
        &set(&["delete_messages"]),
        run,
    )
    .await;
    let ChatToolOutcome::AwaitingOwner(action, _) = outcome else {
        panic!("not held: {outcome:?}");
    };
    assert_eq!(
        action.affected,
        vec![
            "uid 7: boss@corp.test, \"Q3 invoices\"".to_string(),
            "uid 9: boss@corp.test, \"Q3 invoices\"".to_string(),
        ]
    );
    assert_eq!(
        *ran.lock().unwrap(),
        vec!["get_email_by_uid", "get_email_by_uid"],
        "only lookups ran"
    );
}

#[tokio::test]
async fn reads_run_and_tools_not_offered_are_refused() {
    let (ran, run) = recorder();
    let offered = set(&["search_cached_emails"]);
    let read = handle_call("search_cached_emails", json!({}), &offered, &set(&[]), &run).await;
    assert!(
        matches!(read, ChatToolOutcome::Ran(ref line) if line.starts_with("TOOL_RESULT")),
        "{read:?}"
    );
    // send_email was not in this conversation's toolset: refused, not even held.
    let send = handle_call("send_email", json!({}), &offered, &set(&[]), &run).await;
    assert!(matches!(send, ChatToolOutcome::Refused(_)), "{send:?}");
    assert_eq!(*ran.lock().unwrap(), vec!["search_cached_emails"]);
}

#[test]
fn uids_are_read_from_either_form() {
    assert_eq!(uids_in(&json!({"uid": 4})), vec![4]);
    assert_eq!(uids_in(&json!({"uids": [1, 2]})), vec![1, 2]);
    assert_eq!(uids_in(&json!({"uids": "3, 5"})), vec![3, 5]);
    assert!(uids_in(&json!({})).is_empty());
}
