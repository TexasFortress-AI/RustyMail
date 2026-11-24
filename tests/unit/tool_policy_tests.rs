// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A model inside RustyMail may only read, search and draft. Every published
//! tool is classified, so a new tool fails here until someone decides.

use rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format;
use rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format;
use rustymail::dashboard::services::ai::tool_policy::{is_classified, model_may_run, refusal};

#[test]
fn every_published_tool_is_classified() {
    let unclassified: Vec<String> = get_mcp_tools_jsonrpc_format()
        .into_iter()
        .chain(get_mcp_high_level_tools_jsonrpc_format())
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .filter(|name| !is_classified(name))
        .collect();
    assert!(
        unclassified.is_empty(),
        "add these to MODEL_MAY_RUN or OWNER_ONLY in tool_policy.rs: {unclassified:?}"
    );
}

#[test]
fn a_model_cannot_send_delete_move_or_flag() {
    for tool in [
        "send_email",
        "delete_messages",
        "mark_as_deleted",
        "expunge",
        "delete_folder",
        "atomic_move_message",
        "atomic_batch_move",
        "rename_folder",
        "mark_as_read",
        "download_email_attachments",
        "export_evidence",
        "set_tool_calling_model",
        "some_tool_added_next_year",
    ] {
        assert!(!model_may_run(tool), "{tool}");
    }
    for tool in [
        "get_email_by_uid",
        "search_cached_emails",
        "draft_reply",
        "list_accounts",
    ] {
        assert!(model_may_run(tool), "{tool}");
    }
    assert!(refusal("send_email").contains("only the owner"));
}
