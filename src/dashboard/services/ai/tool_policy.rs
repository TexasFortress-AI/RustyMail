// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which tools a model inside RustyMail (the agent, the chat assistant) may
//! run on its own.
//!
//! The model reads email, and email is written by strangers: a message can
//! tell the model to forward invoices or delete a folder, and a small local
//! model may obey. So the model may only read, search and draft (a draft is
//! saved, never sent). Anything that sends, deletes, moves, renames, changes
//! flags, writes to a path it chooses or changes settings needs the owner,
//! whatever the model was told. The list is of what is allowed: a tool not on
//! it, including any tool added later, needs the owner.

/// Tools a model may run without the owner: reads, searches, drafts, the
/// agent itself (which is held to this same list), and status reads.
const MODEL_MAY_RUN: &[&str] = &[
    // Reading and searching mail (bodies are fetched with BODY.PEEK, so
    // reading never marks a message read).
    "list_accounts",
    "list_folders",
    "list_folders_hierarchical",
    "list_cached_emails",
    "get_email_by_uid",
    "get_email_by_index",
    "fetch_emails_with_mime",
    "count_emails_in_folder",
    "get_folder_stats",
    "search_cached_emails",
    "search_by_domain",
    "search_by_attachment_type",
    "filter_emails_by_subject",
    "list_emails_by_flag",
    "get_address_report",
    "get_email_synopsis",
    "batch_get_synopsis",
    "get_email_thread",
    "list_email_attachments",
    "get_attachment_content",
    // Writes only to the temp directory, under a name it builds itself.
    "export_folder_metadata",
    // Drafts are saved to Drafts for the owner to review; nothing is sent.
    "draft_reply",
    "draft_email",
    // The agent, which runs under this same policy.
    "process_email_instructions",
    // Status reads.
    "get_model_configurations",
    "list_jobs",
    "get_job_status",
];

/// Tools only the owner may run (from the dashboard or their own MCP client).
/// Not consulted by the gate, which allows only `MODEL_MAY_RUN`; listed so a
/// test can require every published tool to be classified one way or the
/// other, and a new tool fails CI until someone decides.
pub const OWNER_ONLY: &[&str] = &[
    // Sends mail out.
    "send_email",
    // Deletes or hides mail.
    "delete_messages",
    "mark_as_deleted",
    "undelete_messages",
    "expunge",
    "delete_folder",
    // Moves and renames mail and folders.
    "atomic_move_message",
    "atomic_batch_move",
    "rename_folder",
    "create_folder",
    // Read/unread state: marking read can hide a message from the owner.
    "mark_as_read",
    "mark_as_unread",
    // Writes files to a path the caller chooses, or deletes stored files.
    "download_email_attachments",
    "export_evidence",
    "cleanup_attachments",
    // Changes settings or jobs, or starts server work.
    "set_tool_calling_model",
    "set_drafting_model",
    "cancel_job",
    "sync_emails",
];

/// Whether `tool` is in either list (for the classification test).
pub fn is_classified(tool: &str) -> bool {
    MODEL_MAY_RUN.contains(&tool) || OWNER_ONLY.contains(&tool)
}

/// Whether a model may run `tool` on its own. Anything else (sending,
/// deleting, moving, renaming, flagging, creating folders, choosing file
/// paths, changing settings, cancelling jobs, and every unknown tool) needs
/// the owner's confirmation.
pub fn model_may_run(tool: &str) -> bool {
    MODEL_MAY_RUN.contains(&tool)
}

/// Why a model's call was not run, for the model to read and pass on.
pub fn refusal(tool: &str) -> String {
    format!(
        "'{tool}' was not run: it changes mail, sends it or writes files, so only the owner \
         can do it. Do not retry. Tell the owner what you would do and why."
    )
}
