// tests/unit/nlp_processor_tests.rs
// Unit tests for NLP processor

#[cfg(test)]
mod tests {
    use rustymail::dashboard::services::ai::nlp_processor::{
        NlpProcessor, EmailIntent, ExtractedEntities, McpOperation
    };
    use rustymail::dashboard::services::ai::provider_manager::ProviderManager;

    #[test]
    fn test_pattern_based_intent_detection() {
        let provider_manager = ProviderManager::new();
        let processor = NlpProcessor::new(provider_manager);

        // Test folder operations
        let test_cases = vec![
            ("show me my folders", EmailIntent::ListFolders),
            ("list all folders", EmailIntent::ListFolders),
            ("display mailboxes", EmailIntent::ListFolders),
            ("show unread emails", EmailIntent::ShowUnreadEmails),
            ("list new messages", EmailIntent::ShowUnreadEmails),
            ("help", EmailIntent::Help),
            ("what can you do", EmailIntent::Help),
        ];

        for (query, expected_intent) in test_cases {
            let detected = processor.detect_intent_by_pattern(query);
            assert_eq!(
                detected, expected_intent,
                "Failed for query: '{}'", query
            );
        }
    }

    #[test]
    fn test_entity_extraction_from_json() {
        let provider_manager = ProviderManager::new();
        let processor = NlpProcessor::new(provider_manager);

        let test_json = serde_json::json!({
            "intent": "search_emails",
            "entities": {
                "folders": ["INBOX", "Sent", "Drafts"],
                "senders": ["john@example.com", "jane@example.com"],
                "subjects": ["meeting", "report"],
                "dates": ["yesterday", "last week"],
                "flags": ["unread", "flagged"],
                "counts": [10, 20],
                "search_terms": ["project", "deadline"]
            }
        });

        let entities = processor.parse_entities_from_json(&test_json);

        assert_eq!(entities.folders.len(), 3);
        assert_eq!(entities.folders[0], "INBOX");
        assert_eq!(entities.senders.len(), 2);
        assert_eq!(entities.senders[0], "john@example.com");
        assert_eq!(entities.subjects.len(), 2);
        assert_eq!(entities.dates.len(), 2);
        assert_eq!(entities.flags.len(), 2);
        assert_eq!(entities.counts.len(), 2);
        assert_eq!(entities.counts[0], 10);
        assert_eq!(entities.search_terms.len(), 2);
    }

    #[test]
    fn test_mcp_operation_mapping() {
        let provider_manager = ProviderManager::new();
        let processor = NlpProcessor::new(provider_manager);

        // Test ListFolders intent
        let result = processor.map_to_mcp_operation(
            &EmailIntent::ListFolders,
            &ExtractedEntities::default()
        );
        assert!(result.is_ok());
        let mcp_op = result.unwrap();
        assert_eq!(mcp_op.method, "list_folders");

        // Test ShowUnreadEmails intent
        let mut entities = ExtractedEntities::default();
        entities.folders = vec!["Work".to_string()];

        let result = processor.map_to_mcp_operation(
            &EmailIntent::ShowUnreadEmails,
            &entities
        );
        assert!(result.is_ok());
        let mcp_op = result.unwrap();
        assert_eq!(mcp_op.method, "search_emails");
        assert_eq!(mcp_op.params["folder"], "Work");
        assert_eq!(mcp_op.params["query"], "UNSEEN");

        // Test CreateFolder intent
        let result = processor.map_to_mcp_operation(
            &EmailIntent::CreateFolder("Projects".to_string()),
            &ExtractedEntities::default()
        );
        assert!(result.is_ok());
        let mcp_op = result.unwrap();
        assert_eq!(mcp_op.method, "create_folder");
        assert_eq!(mcp_op.params["name"], "Projects");
    }

    #[test]
    fn test_intent_parsing_from_json() {
        let provider_manager = ProviderManager::new();
        let processor = NlpProcessor::new(provider_manager);

        let test_cases = vec![
            (serde_json::json!({"intent": "list_folders"}), EmailIntent::ListFolders),
            (serde_json::json!({"intent": "show_unread"}), EmailIntent::ShowUnreadEmails),
            (serde_json::json!({"intent": "help"}), EmailIntent::Help),
            (serde_json::json!({"intent": "unknown_intent"}), EmailIntent::Unknown),
            (serde_json::json!({}), EmailIntent::Unknown),
        ];

        for (json, expected_intent) in test_cases {
            let parsed = processor.parse_intent_from_json(&json);
            assert_eq!(parsed, expected_intent);
        }
    }

    #[test]
    fn test_extraction_response_parsing() {
        let provider_manager = ProviderManager::new();
        let processor = NlpProcessor::new(provider_manager);

        // Test successful JSON parsing
        let response = r#"
        Based on the query, here's the extracted information:
        {
            "intent": "list_folders",
            "entities": {
                "folders": ["INBOX"],
                "search_terms": []
            }
        }
        "#;

        let result = processor.parse_extraction_response(response, "show folders");
        assert!(result.is_ok());
        let nlp_result = result.unwrap();
        assert_eq!(nlp_result.intent, EmailIntent::ListFolders);
        assert_eq!(nlp_result.entities.folders.len(), 1);

        // Test fallback on invalid JSON
        let bad_response = "This is not JSON";
        let result = processor.parse_extraction_response(bad_response, "show folders");
        assert!(result.is_ok());
        let nlp_result = result.unwrap();
        assert_eq!(nlp_result.confidence, 0.5); // Lower confidence for fallback
    }

    #[test]
    fn test_default_extracted_entities() {
        let entities = ExtractedEntities::default();

        assert!(entities.folders.is_empty());
        assert!(entities.senders.is_empty());
        assert!(entities.subjects.is_empty());
        assert!(entities.dates.is_empty());
        assert!(entities.flags.is_empty());
        assert!(entities.counts.is_empty());
        assert!(entities.search_terms.is_empty());
    }
}