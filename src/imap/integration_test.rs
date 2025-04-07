// This file contains integration tests that require a live IMAP server
// and credentials defined in a .env file (IMAP_HOST, IMAP_PORT, IMAP_USER, IMAP_PASS).
// Run these tests with: cargo test --features integration_tests

#[cfg(all(test, feature = "integration_tests"))]
mod tests {
    use crate::imap::{ImapClient, ImapError, SearchCriteria, Folder, Email, ImapSession, OwnedMailbox}; // Import needed types
    use crate::config::Settings;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use dotenvy::dotenv;
    use imap_types::flag::Flag;
    use std::panic;
    use std::sync::Arc;

    // Helper to create a unique folder name for testing
    fn unique_test_folder_name(prefix: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        format!("rustymail_test_{}_{}", prefix, timestamp)
    }

    // Helper to get connected session Arc
    async fn connect_real_imap() -> Result<Arc<dyn ImapSession>, String> {
        dotenv().ok();
        // Explicitly check if the env var is loaded after dotenv()
        match std::env::var("APP_IMAP__HOST") {
            Ok(host) => println!("Debug: APP_IMAP__HOST found: {}", host),
            Err(_) => println!("Debug: APP_IMAP__HOST not found after dotenv()!"),
        }
        // Print detailed error before mapping
        let settings = Settings::new(None).map_err(|e| {
            println!("Detailed Config Error: {:?}", e); // Add this line for detailed debug output
            format!("Settings Error: {}", e)
        })?;

        // Use nested imap struct for config -- NO! Use flattened fields now.
        if settings.imap_host.is_empty() || settings.imap_user.is_empty() || settings.imap_pass.is_empty() {
            return Err("IMAP credentials missing in .env".to_string());
        }

        println!("Connecting to {}:{} for user {}", settings.imap_host, settings.imap_port, settings.imap_user);

        // Call connect with timeout
        let session_arc = ImapClient::connect(
            &settings.imap_host,
            settings.imap_port,
            &settings.imap_user,
            &settings.imap_pass,
            Some(Duration::from_secs(15)), // Add timeout
        )
        .await
        .map_err(|e| format!("IMAP Connection/Login Error: {}", e))?;
        
        // Return the Arc<dyn ImapSession>
        Ok(session_arc)
    }
    
    // Wrapper to run tests with cleanup
    fn run_test_with_cleanup<F, Fut>(test_name: &str, test_fn: F)
    where
        F: FnOnce(Arc<dyn ImapSession>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        println!("\n--- Running Test: {} ---", test_name);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        
        let session_arc_result = runtime.block_on(connect_real_imap());
        if let Err(e) = session_arc_result {
            panic!("Failed to connect IMAP for test {}: {}", test_name, e);
        }
        let session_arc = session_arc_result.unwrap();
        
        let session_arc_for_test = session_arc.clone();
        let session_arc_for_cleanup = session_arc;

        // Run the test directly, without catch_unwind
        // Panics in test_fn will stop execution here
        runtime.block_on(async move {
            test_fn(session_arc_for_test).await;
        });

        // Cleanup logic (might not run if test panicked)
        println!("--- Cleaning up after: {} ---", test_name);
        runtime.block_on(async move {
            let cleanup_client = ImapClient::new(session_arc_for_cleanup);
            cleanup_test_folders(&cleanup_client).await;
        });
        
        // Test passed if it didn't panic
        println!("--- Test Passed: {} ---", test_name);
    }

    // Helper to delete test folders
    async fn cleanup_test_folders(client: &ImapClient) {
        let test_prefixes = ["__RustyMailTestFolder__", "__TestRenamed__"];
        match client.list_folders().await {
            Ok(folders) => {
                for folder in folders {
                    if test_prefixes.iter().any(|prefix| folder.name.starts_with(prefix)) {
                        println!("Cleanup: Deleting test folder '{}'", folder.name);
                        // Use Mailbox error variant
                        if let Err(e @ ImapError::Mailbox(_)) = client.delete_folder(&folder.name).await {
                            println!("Cleanup: Error deleting folder '{}': {}", folder.name, e);
                        }
                    }
                }
            }
            Err(e) => println!("Cleanup: Error listing folders: {}", e),
        }
    }

    // --- Tests --- 

    #[test]
    fn test_imap_connect_and_list_folders() {
        run_test_with_cleanup("test_imap_connect_and_list_folders", |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let result = client.list_folders().await;
            assert!(result.is_ok(), "list_folders failed: {:?}", result.err());
            assert!(!result.unwrap().is_empty(), "Expected some folders, found none");
        });
    }

    #[test]
    fn test_imap_create_and_delete_folder() {
        run_test_with_cleanup("test_imap_create_and_delete_folder", |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let folder_name = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
            // Create
            let create_result = client.create_folder(&folder_name).await;
            assert!(create_result.is_ok(), "create_folder failed: {:?}", create_result.err());
            // Verify 
            let folders = client.list_folders().await.expect("List after create failed");
            assert!(folders.iter().any(|f| f.name == folder_name), "Test folder '{}' not found after creation", folder_name);
            // Delete (cleanup is handled by run_test_with_cleanup)
        });
    }

    #[test]
    fn test_imap_select_inbox() {
         run_test_with_cleanup("test_imap_select_inbox", |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let result = client.select_folder("INBOX").await;
            assert!(result.is_ok(), "select_folder(INBOX) failed: {:?}", result.err());
            let mailbox_info = result.unwrap();
            // Pattern match on the Mailbox enum
            match mailbox_info {
                imap_types::mailbox::Mailbox::Inbox => { /* Correct */ }
                imap_types::mailbox::Mailbox::Other(other) => {
                    panic!("Expected Mailbox::Inbox, got Mailbox::Other({:?})", other);
                }
            }
            // Or specifically check name if Other is possible
            // assert!(matches!(mailbox_info, imap_types::mailbox::Mailbox::Inbox)); 
        });
    }

    /* // Comment out append tests for now
    #[test]
    fn test_imap_append_email() {
        let subject_unique = format!("Test Append {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let folder_name = "INBOX"; // Or another suitable test folder
        run_test_with_cleanup("test_imap_append_email", { let subject_clone = subject_unique.clone(); move |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let folder_clone = folder_name.to_string();
            let email_body = format!("From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: {}\r\n\r\nThis is the test email body.\r\n", subject_clone);

            let append_result = client.append(&folder_clone, email_body.as_bytes(), Some(&[Flag::Seen])).await;
            assert!(append_result.is_ok(), "append failed: {:?}", append_result.err());
            
            // Verify by searching 
            tokio::time::sleep(Duration::from_secs(2)).await; // Allow time for index update
            let search_criteria = SearchCriteria::Subject(subject_clone.clone());
            let search_result = client.search_emails(search_criteria).await;
            assert!(search_result.is_ok(), "Search after append failed: {:?}", search_result.err());
            let uids = search_result.unwrap();
            assert!(!uids.is_empty(), "Email with subject '{}' not found after append", subject_clone);

            // Optional: Fetch and verify content
            let fetch_result = client.fetch_emails(uids).await;
            assert!(fetch_result.is_ok(), "Fetch after append failed: {:?}", fetch_result.err());
            let emails = fetch_result.unwrap();
            assert_eq!(emails.len(), 1);
            let email = &emails[0];
            // Fix assertion comparison
            assert_eq!(email.envelope.as_ref().unwrap().subject.as_deref(), Some(subject_clone.as_str()));

            // Cleanup: Delete the appended email (if possible/needed)
        }});
    }
    */

    #[test]
    fn test_imap_rename_folder() {
        let base_name = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let renamed_name = format!("__TestRenamed__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        run_test_with_cleanup("test_imap_rename_folder", { let base_clone = base_name.clone(); let renamed_clone = renamed_name.clone(); move |session_arc| async move {
            let client = ImapClient::new(session_arc);
            // Ensure start state
            let _ = client.delete_folder(&base_clone).await;
            let _ = client.delete_folder(&renamed_clone).await;
            client.create_folder(&base_clone).await.expect("Setup: create failed");
            
            // Rename
            let rename_result = client.rename_folder(&base_clone, &renamed_clone).await;
            assert!(rename_result.is_ok(), "rename_folder failed: {:?}", rename_result.err());

            // Verify old name gone, new name exists
            let folders = client.list_folders().await.expect("List after rename failed");
            assert!(!folders.iter().any(|f| f.name == base_clone), "Old folder name still exists");
            assert!(folders.iter().any(|f| f.name == renamed_clone), "New folder name not found");
        }});
    }
    
    /* // Comment out append tests for now
    #[test]
    fn test_imap_move_email() {
         let subject_unique = format!("Test Move {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         let src_folder = "INBOX";
         let dest_folder = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         run_test_with_cleanup("test_imap_move_email", { let subject_clone = subject_unique.clone(); let dest_clone = dest_folder.clone(); let src_clone = src_folder.to_string(); move |session_arc| async move {
             let client = ImapClient::new(session_arc);
             // Ensure dest folder exists, is empty
             let _ = client.delete_folder(&dest_clone).await;
             client.create_folder(&dest_clone).await.expect("Setup: create dest failed");

             // Append email to source folder
             let email_body = format!("Subject: {}\r\n\r\nTest body.", subject_clone);
             client.append(&src_clone, email_body.as_bytes(), None).await.expect("Setup: append failed");
             tokio::time::sleep(Duration::from_secs(2)).await;
             let uids = client.search_emails(SearchCriteria::Subject(subject_clone.clone())).await.expect("Setup: search failed");
             assert!(!uids.is_empty(), "Setup: email not found");

             // Move email
             let move_result = client.move_email(uids.clone(), &dest_clone).await;
             assert!(move_result.is_ok(), "move_email failed: {:?}", move_result.err());
             tokio::time::sleep(Duration::from_secs(2)).await;

             // Verify email gone from source
             let src_uids = client.search_emails(SearchCriteria::Subject(subject_clone.clone())).await.expect("Verify: search src failed");
             assert!(src_uids.is_empty(), "Email still found in source folder after move");

             // Verify email exists in destination
             client.select_folder(&dest_clone).await.expect("Verify: select dest failed");
             let dest_uids = client.search_emails(SearchCriteria::Subject(subject_clone.clone())).await.expect("Verify: search dest failed");
             assert!(!dest_uids.is_empty(), "Email not found in destination folder after move");
         }});
    }
    */
    
    #[test]
    fn test_imap_delete_non_existent_folder() {
         let folder_name = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         run_test_with_cleanup("test_imap_delete_non_existent_folder", { let folder_clone = folder_name.clone(); move |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let _ = client.delete_folder(&folder_clone).await;
            let delete_result = client.delete_folder(&folder_clone).await;
            assert!(delete_result.is_err(), "Deleting non-existent folder should fail");
             // Use Mailbox error variant
            if let Err(ImapError::Mailbox(e)) = delete_result {
                println!("Delete non-existent folder expected error: {}", e);
            } else if let Err(e) = delete_result { 
                 println!("Delete non-existent folder other error: {}", e);
            } else {
                 panic!("Delete non-existent folder succeeded unexpectedly");
            }
         }});
    }
    
    #[test]
    fn test_imap_select_non_existent_folder() {
         let folder_name = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
          run_test_with_cleanup("test_imap_select_non_existent_folder", { let folder_clone = folder_name.clone(); move |session_arc| async move {
             let client = ImapClient::new(session_arc);
             // Ensure folder does not exist
              let _ = client.delete_folder(&folder_clone).await;
              
             let select_result = client.select_folder(&folder_clone).await;
             assert!(select_result.is_err(), "Select non-existent folder should fail");
             // Expect Mailbox error
             assert!(matches!(select_result.unwrap_err(), ImapError::Mailbox(_)), "Expected Mailbox error for select non-existent");
         }});
    }
    
    #[test]
    fn test_imap_fetch_invalid_uid() {
          run_test_with_cleanup("test_imap_fetch_invalid_uid", |session_arc| async move {
             let client = ImapClient::new(session_arc);
             // Select INBOX first (fetch needs selected mailbox)
             client.select_folder("INBOX").await.expect("Select INBOX failed");
             
             let invalid_uid = vec![9999999]; // Assuming this UID doesn't exist
             let fetch_result = client.fetch_emails(invalid_uid).await;
             assert!(fetch_result.is_ok(), "Fetch invalid UID failed: {:?}", fetch_result.err());
             assert!(fetch_result.unwrap().is_empty(), "Expected empty vec for fetch invalid UID");
        });
    }
    
    /* // Comment out append tests
    #[test]
    fn test_imap_move_to_non_existent_folder() {
         let subject_unique = format!("Test Move Fail {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         let src_folder = "INBOX";
         let dest_folder = format!("__RustyMailTestFolder__{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         run_test_with_cleanup("test_imap_move_to_non_existent_folder", { let subject_clone = subject_unique.clone(); let dest_clone = dest_folder.clone(); let src_clone = src_folder.to_string(); move |session_arc| async move {
             let client = ImapClient::new(session_arc);
             // Ensure dest folder does not exist
             let _ = client.delete_folder(&dest_clone).await;

             // Append email to source folder
             let email_body = format!("Subject: {}\r\n\r\nTest body.", subject_clone);
             client.append(&src_clone, email_body.as_bytes(), None).await.expect("Setup: append failed");
             tokio::time::sleep(Duration::from_secs(2)).await;
             let uids = client.search_emails(SearchCriteria::Subject(subject_clone.clone())).await.expect("Setup: search failed");
             assert!(!uids.is_empty(), "Setup: email not found");

             // Attempt Move to non-existent folder
             let move_result = client.move_email(uids.clone(), &dest_clone).await;
             assert!(move_result.is_err(), "Move to non-existent folder should fail");
             // Expect Mailbox error
             assert!(matches!(move_result.unwrap_err(), ImapError::Mailbox(_)), "Expected Mailbox error for move to non-existent");
         }});
    }
    */
    
    #[test]
    fn test_imap_rename_to_existing_folder() {
         let base_name = format!("__RustyMailTestFolder__Base{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         let existing_name = format!("__RustyMailTestFolder__Existing{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
         run_test_with_cleanup("test_imap_rename_to_existing_folder", { let base_clone = base_name.clone(); let existing_clone = existing_name.clone(); move |session_arc| async move {
             let client = ImapClient::new(session_arc);
             // Ensure start state
             let _ = client.delete_folder(&base_clone).await;
             let _ = client.delete_folder(&existing_clone).await;
             client.create_folder(&base_clone).await.expect("Setup: create base failed");
             client.create_folder(&existing_clone).await.expect("Setup: create existing failed");
             
             // Attempt Rename
             let rename_result = client.rename_folder(&base_clone, &existing_clone).await;
             assert!(rename_result.is_err(), "Rename to existing folder should fail");
             // Expect Mailbox error
             assert!(matches!(rename_result.unwrap_err(), ImapError::Mailbox(_)), "Expected Mailbox error for rename to existing");
         }});
    }
    
    /* // Comment out append tests
    #[test]
    fn test_imap_concurrent_operations() {
        // This test requires careful setup and might be flaky depending on server
        let folder_name = format!("__RustyMailTestFolder__Concurrent{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        run_test_with_cleanup("test_imap_concurrent_operations", { let folder_clone = folder_name.clone(); move |session_arc| async move {
            let client = ImapClient::new(session_arc);
            let client1 = client.clone(); // Clone the client Arc
            let client2 = client.clone();
            let folder_clone1 = folder_clone.clone();
            let folder_clone2 = folder_clone.clone();

            // Ensure folder exists
             let _ = client.delete_folder(&folder_clone).await;
             client.create_folder(&folder_clone).await.expect("Setup: create folder failed");

            // Spawn two tasks trying to append simultaneously (requires append to be working)
            let task1 = tokio::spawn(async move {
                let email_body = b"Subject: Task 1\r\n\r\nBody 1";
                client1.append(&folder_clone1, email_body, None).await
            });
            let task2 = tokio::spawn(async move {
                 let email_body = b"Subject: Task 2\r\n\r\nBody 2";
                 client2.append(&folder_clone2, email_body, None).await
            });

            let (res1, res2) = tokio::join!(task1, task2);
            
            assert!(res1.is_ok() && res1.unwrap().is_ok(), "Task 1 append failed");
            assert!(res2.is_ok() && res2.unwrap().is_ok(), "Task 2 append failed");

            // Verify two emails exist
             client.select_folder(&folder_clone).await.expect("Verify: select failed");
             let uids = client.search_emails(SearchCriteria::All).await.expect("Verify: search failed");
             assert_eq!(uids.len(), 2, "Expected 2 emails after concurrent append");
        }});
    }
    */
} 