// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Integration tests for RustyMail
// This module organizes all integration tests

pub mod agent_tool_gate; // the agent cannot send, delete or move mail
pub mod api_auth;
pub mod api_errors;
pub mod api_rest;
pub mod api_validation;
pub mod cache_after_move; // Moves and deletes drop the source folder's cached rows
pub mod chatbot_integration; // Email Assistant chatbot MCP client integration tests
pub mod connection_pool; // Connection pool integration tests
pub mod dashboard; // Dashboard API endpoints integration tests
pub mod dashboard_auth; // Every /api/dashboard route requires the API key
pub mod dashboard_session; // Browser login: HttpOnly session cookie, same-origin only
pub mod dashboard_smtp_attachments; // Dashboard SMTP and attachment endpoints integration tests
pub mod dashboard_static_site; // Backend serves the built dashboard at /
pub mod fake_imap_flags; // \Seen and CONDSTORE for the fake IMAP server
pub mod fake_imap_server; // TLS fake IMAP server shared by account-routing tests
pub mod flag_changes; // FLAGS-only refresh with CONDSTORE
pub mod mcp_http; // MCP HTTP endpoint integration tests
pub mod mcp_stdio; // MCP stdio proxy integration tests
pub mod message_id_lookup; // finding a message by Message-ID without SEARCH HEADER
pub mod multi_account_sync;
pub mod security_tests; // CORS, origin, auth, path traversal, rate limiting
pub mod smtp_send_lock; // send_email must not deadlock on the account-service lock
pub mod targeted_expunge; // Moves and deletes never purge other \Deleted messages
pub mod tool_dispatch; // High-level tools reach the dispatcher that has them
pub mod triage; // Triage propose against the fake IMAP server
pub mod triage_apply; // Triage apply: verified moves against the fake IMAP server
pub mod triage_junk; // The spam-folder review against the fake IMAP server
pub mod triage_junk_apply; // Applying it: rescues, junk marks, no loop back
pub mod write_tools_account_routing; // Write tools hit the named account's IMAP server; moves are verified
                                     // pub mod test_uid_search_fix; // TODO: Fix ImapSession import
