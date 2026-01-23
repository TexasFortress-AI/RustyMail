// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Integration tests for RustyMail
// This module organizes all integration tests

pub mod dashboard; // Dashboard API endpoints integration tests
pub mod dashboard_smtp_attachments; // Dashboard SMTP and attachment endpoints integration tests
pub mod api_rest;
pub mod api_auth;
pub mod api_validation;
pub mod api_errors;
pub mod multi_account_sync;
pub mod mcp_http; // MCP HTTP endpoint integration tests
pub mod mcp_stdio; // MCP stdio proxy integration tests
pub mod connection_pool; // Connection pool integration tests
pub mod chatbot_integration; // Email Assistant chatbot MCP client integration tests
pub mod security_tests; // Security-focused tests for CORS, origin, auth, path traversal, rate limiting
// pub mod test_uid_search_fix; // TODO: Fix ImapSession import