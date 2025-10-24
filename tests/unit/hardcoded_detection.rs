// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/// Tests to detect and prevent hardcoded account ID values
///
/// This module ensures that the codebase does not contain hardcoded account IDs
/// (like `account_id = 1`) and that all account-related functions properly accept
/// account IDs as parameters.

use regex::Regex;
use std::fs;

/// Test that no Rust source files contain hardcoded account_id = 1 patterns
#[test]
fn test_no_hardcoded_account_id_numeric() {
    let source_files = collect_rust_files("src");
    let pattern = Regex::new(r#"account_id\s*=\s*1\b"#).unwrap();

    let mut violations = Vec::new();

    for file_path in source_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                // Skip comments
                if line.trim().starts_with("//") || line.trim().starts_with("/*") {
                    continue;
                }

                if pattern.is_match(line) {
                    violations.push(format!(
                        "{}:{} - Found hardcoded account_id = 1",
                        file_path, line_num + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found hardcoded account_id values:\n{}",
        violations.join("\n")
    );
}

/// Test that no Rust source files contain hardcoded account_id = "1" string patterns
#[test]
fn test_no_hardcoded_account_id_string() {
    let source_files = collect_rust_files("src");
    let pattern = Regex::new(r#"account_id\s*=\s*"1""#).unwrap();

    let mut violations = Vec::new();

    for file_path in source_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                // Skip comments
                if line.trim().starts_with("//") || line.trim().starts_with("/*") {
                    continue;
                }

                if pattern.is_match(line) {
                    violations.push(format!(
                        "{}:{} - Found hardcoded account_id = \"1\"",
                        file_path, line_num + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found hardcoded account_id string values:\n{}",
        violations.join("\n")
    );
}

/// Test that CacheService methods accept account_id parameter
#[test]
fn test_cache_service_methods_accept_account_id() {
    let cache_service_path = "src/dashboard/services/cache.rs";

    if let Ok(content) = fs::read_to_string(cache_service_path) {
        // Key methods that MUST accept account_id parameter
        let critical_methods = vec![
            "cache_email",
            "get_or_create_folder_for_account",
            "get_cached_emails_for_account",
            "clear_folder_cache",
        ];

        for method_name in critical_methods {
            // Look for the method definition
            let method_pattern = Regex::new(&format!(
                r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+{}\s*\([^)]*account_id\s*:\s*&str[^)]*\)",
                regex::escape(method_name)
            )).unwrap();

            assert!(
                method_pattern.is_match(&content),
                "Method '{}' in CacheService does not accept account_id: &str parameter. \
                All cache methods must accept account_id to support multi-account functionality.",
                method_name
            );
        }
    } else {
        panic!("Could not read cache service file: {}", cache_service_path);
    }
}

/// Test that SyncService methods accept account_id parameter
#[test]
fn test_sync_service_methods_accept_account_id() {
    let sync_service_path = "src/dashboard/services/sync.rs";

    if let Ok(content) = fs::read_to_string(sync_service_path) {
        // Key methods that MUST accept account_id parameter
        let critical_methods = vec![
            "sync_all_folders",
            "sync_folder",
            "sync_folder_with_limit",
            "full_sync_folder",
            "start_idle_monitoring",
        ];

        for method_name in critical_methods {
            // Look for the method definition with account_id parameter
            let method_pattern = Regex::new(&format!(
                r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+{}\s*\([^)]*account_id\s*:\s*&str[^)]*\)",
                regex::escape(method_name)
            )).unwrap();

            assert!(
                method_pattern.is_match(&content),
                "Method '{}' in SyncService does not accept account_id: &str parameter. \
                All sync methods must accept account_id to support multi-account synchronization.",
                method_name
            );
        }
    } else {
        panic!("Could not read sync service file: {}", sync_service_path);
    }
}

/// Test that API handlers don't have hardcoded default account logic
#[test]
fn test_api_handlers_no_default_account() {
    let api_files = collect_rust_files("src/api");

    // Pattern that would indicate hardcoded default account logic
    let suspicious_patterns = vec![
        Regex::new(r#"account_id\s*=\s*"default""#).unwrap(),
        Regex::new(r#"account_id\s*=\s*Some\s*\(\s*"1"\s*\)"#).unwrap(),
        Regex::new(r#"\.unwrap_or\s*\(\s*"1"\s*\)"#).unwrap(),
    ];

    let mut violations = Vec::new();

    for file_path in api_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                // Skip comments
                if line.trim().starts_with("//") || line.trim().starts_with("/*") {
                    continue;
                }

                for pattern in &suspicious_patterns {
                    if pattern.is_match(line) {
                        violations.push(format!(
                            "{}:{} - Suspicious hardcoded default account pattern",
                            file_path, line_num + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found suspicious hardcoded default account patterns:\n{}",
        violations.join("\n")
    );
}

/// Test that database queries use parameterized account_id
#[test]
fn test_database_queries_use_parameters() {
    let source_files = collect_rust_files("src");

    // Pattern for SQL queries with hardcoded account_id values
    let hardcoded_sql_pattern = Regex::new(
        r#"(?i)WHERE\s+account_id\s*=\s*['"]\d+['"]"#
    ).unwrap();

    let mut violations = Vec::new();

    for file_path in source_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if hardcoded_sql_pattern.is_match(line) {
                    violations.push(format!(
                        "{}:{} - Found SQL query with hardcoded account_id",
                        file_path, line_num + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found SQL queries with hardcoded account_id:\n{}",
        violations.join("\n")
    );
}

/// Test that test files use proper account email addresses, not hardcoded IDs
#[test]
fn test_integration_tests_use_email_addresses() {
    let test_files = collect_rust_files("tests/integration");

    // Pattern for hardcoded numeric account IDs in tests
    let hardcoded_test_pattern = Regex::new(
        r#"account_id\s*=\s*"?\d+"?"#
    ).unwrap();

    let mut violations = Vec::new();

    for file_path in test_files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                // Skip comments and email address patterns
                if line.trim().starts_with("//")
                    || line.trim().starts_with("/*")
                    || line.contains("@") // Skip email addresses
                {
                    continue;
                }

                if hardcoded_test_pattern.is_match(line) {
                    violations.push(format!(
                        "{}:{} - Test uses hardcoded numeric account_id instead of email address",
                        file_path, line_num + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found tests with hardcoded numeric account IDs (should use email addresses):\n{}",
        violations.join("\n")
    );
}

/// Helper function to recursively collect all Rust source files in a directory
fn collect_rust_files(dir: &str) -> Vec<String> {
    let mut rust_files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Some(path_str) = path.to_str() {
                    rust_files.push(path_str.to_string());
                }
            } else if path.is_dir() {
                // Recursively collect from subdirectories
                if let Some(subdir) = path.to_str() {
                    rust_files.extend(collect_rust_files(subdir));
                }
            }
        }
    }

    rust_files
}

/// Test that verifies the hardcoded detection framework itself works
#[test]
fn test_hardcoded_detection_framework() {
    // This is a meta-test: verify that our detection functions work correctly

    // Create a temporary test string with a hardcoded pattern
    let test_content = r#"
        let account_id = 1; // This should be detected
        let other_var = 2;  // This should not be detected
    "#;

    let pattern = Regex::new(r#"account_id\s*=\s*1\b"#).unwrap();
    assert!(
        pattern.is_match(test_content),
        "Hardcoded detection regex should match 'account_id = 1' pattern"
    );

    // Verify it doesn't false positive
    let clean_content = r#"
        let account_id = get_account_id();
        let other_var = 1;
    "#;

    assert!(
        !pattern.is_match(clean_content),
        "Hardcoded detection regex should not match when account_id is not hardcoded"
    );
}

#[cfg(test)]
mod account_isolation_tests {

    /// Test that demonstrates proper multi-account pattern
    #[test]
    fn test_proper_multi_account_pattern() {
        // This test documents the CORRECT way to handle account IDs

        // CORRECT: Accept account_id as parameter
        fn fetch_emails(account_id: &str, folder: &str) -> Vec<String> {
            // Implementation would use the provided account_id
            vec![format!("Email for {} in {}", account_id, folder)]
        }

        // Usage with different accounts
        let account1 = "chris@texasfortress.ai";
        let account2 = "shannon@texasfortress.ai";

        let emails1 = fetch_emails(account1, "INBOX");
        let emails2 = fetch_emails(account2, "INBOX");

        assert!(!emails1.is_empty());
        assert!(!emails2.is_empty());
        assert_ne!(emails1[0], emails2[0], "Different accounts should have different data");
    }

    /// Test that demonstrates INCORRECT hardcoded pattern (for documentation)
    #[test]
    #[should_panic(expected = "This is an anti-pattern")]
    fn test_incorrect_hardcoded_pattern() {
        // INCORRECT: Hardcoded account_id (this is what we're preventing)
        fn fetch_emails_wrong(folder: &str) -> Vec<String> {
            let account_id = 1; // ANTI-PATTERN: Hardcoded account ID
            vec![format!("Email for account {} in {}", account_id, folder)]
        }

        // This function can't support multiple accounts!
        let _ = fetch_emails_wrong("INBOX");

        panic!("This is an anti-pattern - functions should accept account_id as parameter");
    }
}
