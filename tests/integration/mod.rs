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
// pub mod test_uid_search_fix; // TODO: Fix ImapSession import 