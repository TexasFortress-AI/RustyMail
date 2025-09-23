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