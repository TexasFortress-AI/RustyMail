// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Tests for the RMCP SDK adapter and legacy tool registry.
//! Validates tool listings, deduplication, field integrity, and ServerHandler impl.

use std::collections::HashSet;
use std::sync::Arc;

// === Legacy Tool Registry Tests ===

#[test]
fn test_legacy_tool_registry_creation() {
    let registry = rustymail::mcp_port::create_mcp_tool_registry();
    let tool_names: Vec<String> = registry.keys().cloned().collect();
    assert!(!tool_names.is_empty(), "Registry should contain tools");
}

#[test]
fn test_legacy_registry_contains_expected_tools() {
    let registry = rustymail::mcp_port::create_mcp_tool_registry();

    let expected_tools = vec![
        "list_folders", "list_folders_hierarchical",
        "search_emails", "fetch_emails_with_mime",
        "atomic_move_message", "atomic_batch_move",
        "mark_as_deleted", "delete_messages", "undelete_messages", "expunge",
        "mark_as_read", "mark_as_unread",
        "list_cached_emails", "get_email_by_uid", "get_email_by_index",
        "count_emails_in_folder", "get_folder_stats", "search_cached_emails",
        "list_accounts", "set_current_account",
        "send_email",
        "list_email_attachments", "download_email_attachments", "cleanup_attachments",
    ];

    for tool_name in &expected_tools {
        assert!(
            registry.get(tool_name).is_some(),
            "Legacy registry should contain tool '{}'",
            tool_name
        );
    }
}

#[test]
fn test_legacy_registry_tool_count() {
    let registry = rustymail::mcp_port::create_mcp_tool_registry();
    let count = registry.keys().count();
    assert_eq!(count, 24, "Legacy registry should have exactly 24 tools, found {}", count);
}

#[test]
fn test_legacy_registry_no_duplicate_names() {
    let registry = rustymail::mcp_port::create_mcp_tool_registry();
    let mut seen = HashSet::new();
    for name in registry.keys() {
        assert!(seen.insert(name.clone()), "Duplicate tool name in legacy registry: {}", name);
    }
}

// === Low-Level Tool Definition Tests ===

#[test]
fn test_low_level_tools_jsonrpc_format() {
    let tools = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    assert_eq!(tools.len(), 34, "Should have 34 low-level tools, found {}", tools.len());
}

#[test]
fn test_low_level_tools_have_required_fields() {
    let tools = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();

    for tool in &tools {
        let name = tool.get("name").and_then(|v| v.as_str());
        assert!(name.is_some(), "Tool missing 'name' field: {:?}", tool);

        let desc = tool.get("description").and_then(|v| v.as_str());
        assert!(desc.is_some(), "Tool '{}' missing 'description'", name.unwrap_or("?"));
        assert!(!desc.unwrap().is_empty(), "Tool '{}' has empty description", name.unwrap_or("?"));

        let schema = tool.get("inputSchema");
        assert!(schema.is_some(), "Tool '{}' missing 'inputSchema'", name.unwrap_or("?"));
        assert!(schema.unwrap().is_object(), "Tool '{}' inputSchema is not an object", name.unwrap_or("?"));
    }
}

#[test]
fn test_low_level_tools_no_duplicates() {
    let tools = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let mut seen = HashSet::new();
    for tool in &tools {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        assert!(seen.insert(name.to_string()), "Duplicate low-level tool: {}", name);
    }
}

#[test]
fn test_low_level_tools_input_schema_has_properties() {
    let tools = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();

    for tool in &tools {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let schema = tool.get("inputSchema").unwrap();

        // Every inputSchema should declare type: "object"
        let schema_type = schema.get("type").and_then(|v| v.as_str());
        assert_eq!(
            schema_type, Some("object"),
            "Tool '{}' inputSchema type should be 'object'", name
        );

        // Every inputSchema should have a properties field
        assert!(
            schema.get("properties").is_some(),
            "Tool '{}' inputSchema missing 'properties'", name
        );
    }
}

// === High-Level Tool Definition Tests ===

#[test]
fn test_high_level_tools_jsonrpc_format() {
    let tools = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();
    assert_eq!(tools.len(), 21, "Should have 21 high-level tools, found {}", tools.len());
}

#[test]
fn test_high_level_tools_have_required_fields() {
    let tools = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

    for tool in &tools {
        let name = tool.get("name").and_then(|v| v.as_str());
        assert!(name.is_some(), "High-level tool missing 'name': {:?}", tool);

        let desc = tool.get("description").and_then(|v| v.as_str());
        assert!(desc.is_some(), "High-level tool '{}' missing 'description'", name.unwrap_or("?"));

        let schema = tool.get("inputSchema");
        assert!(schema.is_some(), "High-level tool '{}' missing 'inputSchema'", name.unwrap_or("?"));
    }
}

#[test]
fn test_high_level_tools_no_duplicates() {
    let tools = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();
    let mut seen = HashSet::new();
    for tool in &tools {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        assert!(seen.insert(name.to_string()), "Duplicate high-level tool: {}", name);
    }
}

#[test]
fn test_high_level_contains_expected_categories() {
    let tools = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();
    let names: HashSet<String> = tools.iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    // Agentic tools
    assert!(names.contains("process_email_instructions"), "Missing agentic tool");
    assert!(names.contains("draft_reply"), "Missing draft_reply");
    assert!(names.contains("draft_email"), "Missing draft_email");

    // Discovery tools (original)
    assert!(names.contains("list_accounts"), "Missing list_accounts");
    assert!(names.contains("list_folders_hierarchical"), "Missing list_folders_hierarchical");
    assert!(names.contains("list_cached_emails"), "Missing list_cached_emails");
    assert!(names.contains("get_email_by_uid"), "Missing get_email_by_uid");
    assert!(names.contains("search_cached_emails"), "Missing search_cached_emails");
    assert!(names.contains("get_folder_stats"), "Missing get_folder_stats");

    // Enhanced discovery tools (newly added)
    assert!(names.contains("get_email_synopsis"), "Missing get_email_synopsis");
    assert!(names.contains("get_email_thread"), "Missing get_email_thread");
    assert!(names.contains("search_by_domain"), "Missing search_by_domain");
    assert!(names.contains("list_emails_by_flag"), "Missing list_emails_by_flag");
    assert!(names.contains("get_address_report"), "Missing get_address_report");
    assert!(names.contains("sync_emails"), "Missing sync_emails");

    // Configuration tools
    assert!(names.contains("get_model_configurations"), "Missing get_model_configurations");
    assert!(names.contains("set_tool_calling_model"), "Missing set_tool_calling_model");
    assert!(names.contains("set_drafting_model"), "Missing set_drafting_model");

    // Job management tools
    assert!(names.contains("list_jobs"), "Missing list_jobs");
    assert!(names.contains("get_job_status"), "Missing get_job_status");
    assert!(names.contains("cancel_job"), "Missing cancel_job");
}

// === SDK Adapter list_tools Deduplication Tests ===

#[test]
fn test_sdk_tool_listing_deduplication() {
    // Simulate the same logic used in SdkMcpAdapter::list_tools()
    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let high_level = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

    let mut seen_names = HashSet::new();
    let mut combined_count = 0;

    for tool_json in low_level.iter().chain(high_level.iter()) {
        let name = tool_json.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        if seen_names.insert(name.to_string()) {
            combined_count += 1;
        }
    }

    // Low-level has 34, high-level has 21, but many high-level tools overlap
    assert!(combined_count > 34, "Combined should be more than just low-level (got {})", combined_count);
    assert!(combined_count < 34 + 21, "Combined should be deduplicated (got {})", combined_count);
    assert_eq!(combined_count, seen_names.len(), "Count should match unique names");
}

#[test]
fn test_sdk_tool_listing_includes_all_low_level() {
    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let high_level = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

    let mut seen_names = HashSet::new();
    for tool_json in low_level.iter().chain(high_level.iter()) {
        let name = tool_json.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        seen_names.insert(name.to_string());
    }

    // Every low-level tool must be in the combined set
    for tool in &low_level {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        assert!(seen_names.contains(name), "Low-level tool '{}' missing from combined set", name);
    }
}

#[test]
fn test_sdk_tool_listing_includes_all_high_level() {
    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let high_level = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

    let mut seen_names = HashSet::new();
    for tool_json in low_level.iter().chain(high_level.iter()) {
        let name = tool_json.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        seen_names.insert(name.to_string());
    }

    // Every high-level tool must be in the combined set
    for tool in &high_level {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        assert!(seen_names.contains(name), "High-level tool '{}' missing from combined set", name);
    }
}

// === RMCP Tool Struct Conversion Tests ===

#[test]
fn test_rmcp_tool_conversion_from_json() {
    use rmcp::model::Tool;

    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let high_level = rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format();

    let mut seen_names = HashSet::new();
    let mut items: Vec<Tool> = Vec::new();

    for tool_json in low_level.iter().chain(high_level.iter()) {
        let name = tool_json.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        if !seen_names.insert(name.to_string()) {
            continue;
        }
        let description = tool_json.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let input_schema = tool_json.get("inputSchema")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        items.push(Tool {
            name: name.to_string().into(),
            title: Some(name.to_string()),
            description: Some(description.to_string().into()),
            input_schema: Arc::new(input_schema),
            output_schema: None,
            execution: None,
            icons: None,
            annotations: None,
            meta: None,
        });
    }

    // Verify all converted tools have non-empty names and descriptions
    for tool in &items {
        assert!(!tool.name.is_empty(), "Converted tool has empty name");
        assert!(tool.description.is_some(), "Tool '{}' missing description after conversion", tool.name);
        let desc = tool.description.as_ref().unwrap();
        assert!(!desc.is_empty(), "Tool '{}' has empty description after conversion", tool.name);
    }
}

// === ServerHandler get_info Tests ===

#[test]
fn test_server_handler_get_info() {
    use rmcp::ServerHandler;

    // McpPortState::default() -> SessionManager::default() -> Settings::default()
    // requires these env vars to be set (they won't be used, just read during init)
    std::env::set_var("IMAP_HOST", "test.example.com");
    std::env::set_var("RUSTYMAIL_API_KEY", "test-key");
    std::env::set_var("REST_HOST", "127.0.0.1");
    std::env::set_var("REST_PORT", "19876");
    std::env::set_var("SSE_HOST", "127.0.0.1");
    std::env::set_var("SSE_PORT", "19877");
    std::env::set_var("DASHBOARD_PORT", "19878");

    // Create a mock session factory that always errors (we won't use it)
    let factory = rustymail::prelude::CloneableImapSessionFactory::new(
        Box::new(|| {
            Box::pin(async {
                Err(rustymail::imap::ImapError::Connection("test mock".to_string()))
            })
        })
    );

    let service = rustymail::mcp::adapters::sdk::RustyMailService::new(factory);
    let info = service.get_info();

    assert_eq!(info.server_info.name, "RustyMail MCP Server");
    assert_eq!(info.server_info.version, "0.1.0");
    assert_eq!(info.server_info.title, Some("RustyMail MCP".to_string()));
    assert_eq!(info.server_info.description, Some("IMAP email client with MCP interface".to_string()));
    assert!(info.capabilities.tools.is_some(), "Server should declare tool capabilities");
    assert!(info.instructions.is_some(), "Server should have instructions");
}

// === Cross-Layer Consistency Tests ===

#[test]
fn test_high_level_browsing_tools_exist_in_low_level() {
    // High-level browsing tools that delegate to execute_mcp_tool_inner must exist in low-level
    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let low_level_names: HashSet<String> = low_level.iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let delegated_tools = vec![
        "list_accounts", "list_folders_hierarchical", "list_cached_emails",
        "get_email_by_uid", "search_cached_emails", "get_folder_stats",
        "get_email_synopsis", "get_email_thread", "search_by_domain",
        "list_emails_by_flag", "get_address_report", "sync_emails",
    ];

    for tool_name in &delegated_tools {
        assert!(
            low_level_names.contains(*tool_name),
            "High-level delegated tool '{}' must exist in low-level tools",
            tool_name
        );
    }
}

#[test]
fn test_legacy_registry_is_subset_of_low_level() {
    // Every tool in the legacy mcp_port registry should also be in low-level handlers,
    // EXCEPT known legacy-only tools that do live IMAP operations with no cache equivalent.
    // The legacy registry (mcp_port.rs) is dead code that has diverged from handlers.rs.
    let legacy_only_tools: HashSet<&str> = ["search_emails"].into_iter().collect();

    let registry = rustymail::mcp_port::create_mcp_tool_registry();
    let low_level = rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();
    let low_level_names: HashSet<String> = low_level.iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    for name in registry.keys() {
        if legacy_only_tools.contains(name.as_str()) {
            continue; // Known divergence: live IMAP tool with no cache-based equivalent
        }
        assert!(
            low_level_names.contains(name),
            "Legacy registry tool '{}' should exist in low-level tool definitions",
            name
        );
    }
}
