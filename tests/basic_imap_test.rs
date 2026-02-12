// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Basic test to verify IMAP operations compile and have correct signatures
use rustymail::prelude::*;

#[test]
fn test_search_criteria_creation() {
    // Test that SearchCriteria can be created and formatted correctly
    let criteria = SearchCriteria::And(vec![
        SearchCriteria::From("test@example.com".to_string()),
        SearchCriteria::Subject("Test Subject".to_string()),
    ]);

    let criteria_str = criteria.to_string();
    assert_eq!(criteria_str, "(FROM \"test@example.com\" SUBJECT \"Test Subject\")");
}

#[test]
fn test_mime_structures() {
    // Test that MIME structures can be created
    let content_type = ContentType {
        main_type: "text".to_string(),
        sub_type: "plain".to_string(),
        parameters: std::collections::HashMap::new(),
    };

    assert_eq!(content_type.main_type, "text");
    assert_eq!(content_type.sub_type, "plain");
}

#[test]
fn test_folder_hierarchy() {
    // Test folder hierarchy building
    let folder_data = vec![
        ("INBOX".to_string(), Some("/".to_string()), vec![]),
        ("INBOX/Sent".to_string(), Some("/".to_string()), vec![]),
        ("INBOX/Drafts".to_string(), Some("/".to_string()), vec![]),
    ];

    let folders = Folder::build_hierarchy(folder_data);

    // Should have one root folder (INBOX)
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "INBOX");

    // INBOX should have 2 children
    assert_eq!(folders[0].children.len(), 2);
}

#[test]
fn test_flag_operations() {
    // Test FlagOperation enum
    let add_op = FlagOperation::Add;
    let _remove_op = FlagOperation::Remove;
    let _set_op = FlagOperation::Set;

    // Just verify they can be created and used in match expressions
    match add_op {
        FlagOperation::Add => assert!(true),
        _ => assert!(false),
    }
}

/// Regression test: IMAP fetches must use BODY.PEEK[] to avoid setting \Seen flag.
/// See bug report: "Read/Unread Status Discrepancy Between MCP Interface and Web UI"
/// Using BODY[] instead of BODY.PEEK[] causes the server to mark emails as read
/// as a side effect of caching/syncing.
#[test]
fn test_imap_fetch_uses_peek_to_preserve_unseen_flag() {
    let source = std::fs::read_to_string("src/imap/session.rs")
        .expect("Failed to read src/imap/session.rs");

    // Find all uid_fetch calls and ensure none use bare BODY[] without PEEK
    for (line_num, line) in source.lines().enumerate() {
        if line.contains("uid_fetch") && line.contains("BODY[]") && !line.contains("BODY.PEEK[]") {
            panic!(
                "src/imap/session.rs line {}: uid_fetch uses BODY[] instead of BODY.PEEK[]. \
                 This will set the \\Seen flag on the server as a side effect of fetching. \
                 Use BODY.PEEK[] to preserve the original flag state.",
                line_num + 1
            );
        }
    }
}