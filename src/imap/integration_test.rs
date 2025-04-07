// This file contains integration tests that require a live IMAP server
// and credentials defined in a .env file (IMAP_HOST, IMAP_PORT, IMAP_USER, IMAP_PASS).
// Run these tests with: cargo test --features integration_tests

#[cfg(all(test, feature = "integration_tests"))]
mod tests {
    use crate::imap::{ImapClient, ImapError, SearchCriteria};
    use crate::config::Settings; // To load .env
    use std::time::{SystemTime, UNIX_EPOCH};
    use dotenvy::dotenv;
    use imap_types::flag::Flag; // Needed for append
    use std::panic; // To catch panics for cleanup

    // Helper to create a unique folder name for testing
    fn unique_test_folder_name(prefix: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        format!("rustymail_test_{}_{}", prefix, timestamp)
    }

    // Helper function to connect to the real IMAP server using .env credentials
    async fn connect_real_imap() -> Result<ImapClient, Box<dyn std::error::Error>> {
        dotenv().ok(); // Load .env file
        let settings = Settings::new()?;
        
        // Check if required settings are present
        if settings.imap_host.is_empty() || settings.imap_user.is_empty() || settings.imap_pass.is_empty() {
            return Err("IMAP credentials (IMAP_HOST, IMAP_USER, IMAP_PASS) not found in .env or environment".into());
        }

        println!("Connecting to {}:{} for user {}", settings.imap_host, settings.imap_port, settings.imap_user);

        let client = ImapClient::connect(
            &settings.imap_host,
            settings.imap_port,
            &settings.imap_user,
            &settings.imap_pass,
        ).await?;
        Ok(client)
    }

    // Helper to run test logic and ensure logout/cleanup even on panic or connection failure
    async fn run_test_with_cleanup<F, Fut>(test_name: &str, test_logic: F)
    where
        F: FnOnce(ImapClient) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        println!("Running {}...", test_name);
        let connect_result = connect_real_imap().await;

        match connect_result {
            Ok(mut client) => {
                 // Use a flag to track if the main logic panicked
                 let mut panicked = false;
                let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    // Need to block here because catch_unwind is not async-aware
                    // This is okay for tests where we control the async runtime.
                     tokio::runtime::Handle::current().block_on(async { test_logic(client).await })
                }));

                // Check the result of the test logic execution
                let final_result = match result {
                    Ok(inner_result) => inner_result, // Test logic completed (may have returned Ok or Err)
                    Err(panic_payload) => {
                         panicked = true;
                         Err(Box::new(ImapError::Internal(format!("Test panicked: {:?}", panic_payload))) as Box<dyn std::error::Error>)
                     }
                 };

                 // Ensure logout even if test panics or fails
                 // Reconnecting might be necessary if the panic left the session unusable
                 match connect_real_imap().await {
                     Ok(mut client_for_logout) => {
                         if let Err(e) = client_for_logout.logout().await {
                             eprintln!("{} Warning: Logout failed after test execution: {:?}", test_name, e);
                         } else {
                             println!("{} Logout successful.", test_name);
                         }
                     }
                     Err(e) => {
                          eprintln!("{} Warning: Re-connection failed during cleanup logout: {:?}", test_name, e);
                      }
                  }

                 // Report final status
                 if panicked {
                     // If we caught a panic, resume unwinding to fail the test properly
                     panic!("{} panicked during execution. See panic payload above.", test_name);
                 } else {
                     match final_result {
                        Ok(()) => println!("{} completed successfully.", test_name),
                        Err(e) => panic!("{} failed: {:?}", test_name, e), // Fail test on logical error
                     }
                 }
            }
            Err(e) => {
                 eprintln!("{} skipped: Could not connect to IMAP server: {}", test_name, e);
                 // Consider this a pass if connection fails, as the environment might not be set up
                 assert!(true, "Skipping test due to connection failure");
            }
        }
    }


    #[tokio::test]
    async fn test_imap_connect_and_list_folders() {
        run_test_with_cleanup("test_imap_connect_and_list_folders", |mut client| async move {
            let folders = client.list_folders().await?;
            println!("Successfully listed {} folders.", folders.len());
            assert!(!folders.is_empty(), "Expected to list at least one folder (e.g., INBOX)");
            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_create_and_delete_folder() {
        let test_folder = unique_test_folder_name("crdel");
        let folder_clone = test_folder.clone(); // Clone for use in closure

        run_test_with_cleanup("test_imap_create_and_delete_folder", move |mut client| async move {
            println!("Creating folder: {}", folder_clone);
            
            // Ensure folder doesn't exist initially (ignore error if it doesn't)
            client.delete_folder(&folder_clone).await.ok();

            // Create the folder
            client.create_folder(&folder_clone).await?;
            println!("Folder '{}' created successfully.", folder_clone);

            // Verify folder exists by listing
            let folders = client.list_folders().await?;
            assert!(folders.iter().any(|f| f.name == folder_clone), "Test folder '{}' not found after creation", folder_clone);
            println!("Folder '{}' found in list.", folder_clone);

            // Delete the folder
            client.delete_folder(&folder_clone).await?;
            println!("Folder '{}' deleted successfully.", folder_clone);

            // Verify folder is gone by listing again
            let folders_after_delete = client.list_folders().await?;
            assert!(!folders_after_delete.iter().any(|f| f.name == folder_clone), "Test folder '{}' still exists after deletion", folder_clone);
            println!("Folder '{}' confirmed deleted.", folder_clone);
            
            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_select_inbox() {
         run_test_with_cleanup("test_imap_select_inbox", |mut client| async move {
            let mailbox_info = client.select_folder("INBOX").await?;
            println!("Selected INBOX: {:?}", mailbox_info);
            assert_eq!(mailbox_info.mailbox.to_string(), "INBOX");
            // Note: Some servers might report 0 exists if empty, relax this check
            // assert!(mailbox_info.exists > 0, "INBOX should have at least one message (usually)");
            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_append_and_search_email() {
        let test_folder = unique_test_folder_name("appendsrch");
        let folder_clone = test_folder.clone();
        let unique_subject = format!("Rustymail Test Append {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let subject_clone = unique_subject.clone();

        run_test_with_cleanup("test_imap_append_and_search_email", move |mut client| async move {
             // Ensure test folder exists and is clean
            client.delete_folder(&folder_clone).await.ok();
            client.create_folder(&folder_clone).await?;
            println!("Created test folder: {}", folder_clone);

            // Append a test email
            let email_body = format!(
                "From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: {}\r\n\r\nThis is a test email body.\r\n",
                subject_clone
            );
            let append_result = client.append(&folder_clone, email_body.as_bytes(), Some(&[Flag::Seen])).await?;
            println!("Appended email to {}, UID assigned: {:?}", folder_clone, append_result);
            // We might not always get a UID back depending on server support
            // assert!(append_result.is_some(), "Expected UID from APPEND (UIDPLUS)");

            // Select the folder
            client.select_folder(&folder_clone).await?;
            println!("Selected folder {}", folder_clone);

            // Search for the email by subject
             let search_criteria = SearchCriteria::Subject(subject_clone.clone());
             let uids = client.search_emails(search_criteria).await?;
             println!("Search results for subject '{}': {:?}", subject_clone, uids);
             assert_eq!(uids.len(), 1, "Expected to find exactly one email with the unique subject");
             
             // Search for all emails in the folder
             let all_uids = client.search_emails(SearchCriteria::All).await?;
             assert_eq!(all_uids.len(), 1, "Expected exactly one email in the test folder");

            // Cleanup: Delete the folder
            client.delete_folder(&folder_clone).await?;
            println!("Cleaned up test folder: {}", folder_clone);

            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_fetch_email() {
        let test_folder = unique_test_folder_name("fetch");
        let folder_clone = test_folder.clone();
        let unique_subject = format!("Rustymail Test Fetch {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let subject_clone = unique_subject.clone();

         run_test_with_cleanup("test_imap_fetch_email", move |mut client| async move {
            // Setup: Create folder and append email
             client.delete_folder(&folder_clone).await.ok();
             client.create_folder(&folder_clone).await?;
             let email_body = format!("Subject: {}\r\n\r\nTest body", subject_clone);
             client.append(&folder_clone, email_body.as_bytes(), None).await?;
             client.select_folder(&folder_clone).await?;
             let uids = client.search_emails(SearchCriteria::All).await?;
             assert_eq!(uids.len(), 1);
             let target_uid = uids[0];

            // Fetch the email
            let fetched_emails = client.fetch_emails(vec![target_uid]).await?;
            assert_eq!(fetched_emails.len(), 1);
            let email = &fetched_emails[0];
            println!("Fetched email: {:?}", email);

            assert_eq!(email.uid, target_uid);
            assert!(email.envelope.is_some());
            assert_eq!(email.envelope.as_ref().unwrap().subject.as_deref(), Some(&subject_clone));

            // Cleanup
            client.delete_folder(&folder_clone).await?;
            println!("Cleaned up test folder: {}", folder_clone);
             Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_rename_folder() {
        let old_folder = unique_test_folder_name("rename_old");
        let new_folder = unique_test_folder_name("rename_new");
        let old_clone = old_folder.clone();
        let new_clone = new_folder.clone();

        run_test_with_cleanup("test_imap_rename_folder", move |mut client| async move {
            // Setup: Create the old folder, ensure new one doesn't exist
            client.delete_folder(&old_clone).await.ok();
            client.delete_folder(&new_clone).await.ok();
            client.create_folder(&old_clone).await?;
            println!("Created folder for rename: {}", old_clone);

            // Rename
            client.rename_folder(&old_clone, &new_clone).await?;
            println!("Renamed {} to {}", old_clone, new_clone);

            // Verify: Old folder gone, new folder exists
            let folders = client.list_folders().await?;
            assert!(!folders.iter().any(|f| f.name == old_clone), "Old folder still exists after rename");
            assert!(folders.iter().any(|f| f.name == new_clone), "New folder not found after rename");
            println!("Rename verified via folder list.");

            // Cleanup: Delete the new folder
            client.delete_folder(&new_clone).await?;
             println!("Cleaned up renamed folder: {}", new_clone);
            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_move_email() {
        let source_folder = unique_test_folder_name("move_src");
        let dest_folder = unique_test_folder_name("move_dest");
        let src_clone = source_folder.clone();
        let dest_clone = dest_folder.clone();
        let unique_subject = format!("Rustymail Test Move {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let subject_clone = unique_subject.clone();

        run_test_with_cleanup("test_imap_move_email", move |mut client| async move {
            // Setup: Create folders and append email to source
            client.delete_folder(&src_clone).await.ok();
            client.delete_folder(&dest_clone).await.ok();
            client.create_folder(&src_clone).await?;
            client.create_folder(&dest_clone).await?;
            let email_body = format!("Subject: {}\r\n\r\nTest move body", subject_clone);
            client.append(&src_clone, email_body.as_bytes(), None).await?;

            // Get UID from source folder
            client.select_folder(&src_clone).await?;
            let uids_in_source = client.search_emails(SearchCriteria::All).await?;
            assert_eq!(uids_in_source.len(), 1, "Email not found in source before move");
            let uid_to_move = uids_in_source[0];

            // Move the email
            client.move_email(vec![uid_to_move], &dest_clone).await?;
            println!("Moved email UID {} from {} to {}", uid_to_move, src_clone, dest_clone);

            // Verify: Email gone from source
            // Note: Selecting the folder again might be necessary if the server state changes significantly after move
            client.select_folder(&src_clone).await?; 
            let uids_after_move_src = client.search_emails(SearchCriteria::All).await?;
            assert!(uids_after_move_src.is_empty(), "Email still found in source folder after move");
            println!("Verified email gone from source {}", src_clone);

            // Verify: Email present in destination
            client.select_folder(&dest_clone).await?;
            let uids_in_dest = client.search_emails(SearchCriteria::All).await?;
            assert_eq!(uids_in_dest.len(), 1, "Email not found in destination folder after move");
            // Optionally, fetch and check subject if needed, but UID presence is usually sufficient
            println!("Verified email present in destination {}", dest_clone);

            // Cleanup
            client.delete_folder(&src_clone).await?;
            client.delete_folder(&dest_clone).await?;
            println!("Cleaned up move test folders");
            Ok(())
        }).await;
    }

    #[tokio::test]
    async fn test_imap_delete_non_empty_folder_workflow() {
        let test_folder = unique_test_folder_name("del_nonempty");
        let folder_clone = test_folder.clone();
        let unique_subject = format!("Rustymail Test Delete NonEmpty {}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());
        let subject_clone = unique_subject.clone();

        run_test_with_cleanup("test_imap_delete_non_empty_folder_workflow", move |mut client| async move {
            // Setup: Create folder and append email
            client.delete_folder(&folder_clone).await.ok();
            client.create_folder(&folder_clone).await?;
            let email_body = format!("Subject: {}\r\n\r\nTest delete non-empty", subject_clone);
            client.append(&folder_clone, email_body.as_bytes(), None).await?;
            println!("Setup folder {} with one email", folder_clone);

            // Attempt to delete non-empty folder (should fail)
            let delete_result = client.delete_folder(&folder_clone).await;
            assert!(delete_result.is_err(), "Expected deletion of non-empty folder to fail");
            if let Err(ImapError::Protocol(e)) | Err(ImapError::Mailbox(e)) = delete_result {
                 // Check for specific server errors like "Mailbox has inferior hierarchical names" or similar
                 // This can vary between servers. A general protocol/mailbox error check is okay here.
                 println!("Correctly failed to delete non-empty folder: {}", e);
            } else if let Err(e) = delete_result {
                 // Handle other potential ImapError variants if necessary
                 panic!("Deletion failed with unexpected error type: {:?}", e)
            }
            println!("Verified non-empty folder deletion failed as expected.");

            // Get UID and move email out (e.g., to INBOX)
            client.select_folder(&folder_clone).await?;
            let uids = client.search_emails(SearchCriteria::All).await?;
            assert_eq!(uids.len(), 1);
            let uid_to_move = uids[0];
            // Ensure INBOX exists (should always be true)
            let folders = client.list_folders().await?;
            assert!(folders.iter().any(|f| f.name == "INBOX"));
            client.move_email(vec![uid_to_move], "INBOX").await?;
             println!("Moved email from {} to INBOX", folder_clone);

            // Attempt to delete the now-empty folder (should succeed)
            client.delete_folder(&folder_clone).await?;
            println!("Successfully deleted the now-empty folder {}", folder_clone);

            // Verify folder is gone
            let folders_after_delete = client.list_folders().await?;
            assert!(!folders_after_delete.iter().any(|f| f.name == folder_clone), "Test folder still exists after being emptied and deleted");
            println!("Verified folder is gone.");
            
            // Note: We don't delete the email moved to INBOX to avoid polluting a critical folder

            Ok(())
        }).await;
    }

    // --- Error Condition Tests ---

    #[tokio::test]
    async fn test_imap_select_non_existent_folder() {
        let non_existent_folder = unique_test_folder_name("nonexistent_select");
        let folder_clone = non_existent_folder.clone();
         run_test_with_cleanup("test_imap_select_non_existent_folder", move |mut client| async move {
            let result = client.select_folder(&folder_clone).await;
            println!("Attempted to select non-existent folder '{}', result: {:?}", folder_clone, result);
            assert!(result.is_err(), "Expected selecting non-existent folder to fail");
            // Optionally, check the specific error type (e.g., ImapError::Mailbox)
            Ok(())
        }).await;
    }

    // Note: search/fetch without select depends on the session implementation.
    // The current TlsImapSession likely returns an error from the underlying library.
    // A test without prior `select_folder` could be added, but might be redundant if covered by unit tests.

    #[tokio::test]
    async fn test_imap_fetch_invalid_uid() {
        let test_folder = unique_test_folder_name("fetch_invalid");
        let folder_clone = test_folder.clone();
         run_test_with_cleanup("test_imap_fetch_invalid_uid", move |mut client| async move {
            // Setup
             client.delete_folder(&folder_clone).await.ok();
             client.create_folder(&folder_clone).await?;
             client.select_folder(&folder_clone).await?;

            // Attempt to fetch a clearly invalid UID
             let invalid_uid = 999_999_999u32;
             let result = client.fetch_emails(vec![invalid_uid]).await?;
            println!("Attempted to fetch invalid UID {}, result count: {}", invalid_uid, result.len());
             // Fetching a non-existent UID usually returns an empty list, not an error.
             assert!(result.is_empty(), "Expected fetching invalid UID to return empty list");

            // Cleanup
            client.delete_folder(&folder_clone).await?;
            Ok(())
        }).await;
    }

     #[tokio::test]
    async fn test_imap_move_to_non_existent_folder() {
        let source_folder = unique_test_folder_name("move_src_err");
        let non_existent_dest = unique_test_folder_name("move_dest_nonexistent");
        let src_clone = source_folder.clone();
        let dest_clone = non_existent_dest.clone();

        run_test_with_cleanup("test_imap_move_to_non_existent_folder", move |mut client| async move {
            // Setup: Create source folder and email
            client.delete_folder(&src_clone).await.ok();
            client.create_folder(&src_clone).await?;
            let email_body = b"Subject: Test move error\r\n\r\nBody";
            client.append(&src_clone, email_body, None).await?;
            client.select_folder(&src_clone).await?;
            let uids = client.search_emails(SearchCriteria::All).await?;
            assert_eq!(uids.len(), 1);
            let uid_to_move = uids[0];

            // Ensure destination does not exist
            client.delete_folder(&dest_clone).await.ok(); 

            // Attempt to move to non-existent destination
            let move_result = client.move_email(vec![uid_to_move], &dest_clone).await;
            println!("Attempted to move to non-existent folder '{}', result: {:?}", dest_clone, move_result);
            assert!(move_result.is_err(), "Expected move to non-existent folder to fail");
            // Check error type (might be Mailbox or Protocol depending on server)

             // Cleanup
             client.delete_folder(&src_clone).await?;
             Ok(())
         }).await;
    }

    #[tokio::test]
    async fn test_imap_rename_to_existing_folder() {
        let folder1 = unique_test_folder_name("rename_exist1");
        let folder2 = unique_test_folder_name("rename_exist2");
        let f1_clone = folder1.clone();
        let f2_clone = folder2.clone();

        run_test_with_cleanup("test_imap_rename_to_existing_folder", move |mut client| async move {
            // Setup: Create both folders
            client.delete_folder(&f1_clone).await.ok();
            client.delete_folder(&f2_clone).await.ok();
            client.create_folder(&f1_clone).await?;
            client.create_folder(&f2_clone).await?;
            println!("Created folders: {}, {}", f1_clone, f2_clone);

            // Attempt to rename folder1 to folder2 (which exists)
            let rename_result = client.rename_folder(&f1_clone, &f2_clone).await;
            println!("Attempted to rename {} to existing {}, result: {:?}", f1_clone, f2_clone, rename_result);
            assert!(rename_result.is_err(), "Expected renaming to existing folder name to fail");
            // Check error type

            // Cleanup
            client.delete_folder(&f1_clone).await?;
            client.delete_folder(&f2_clone).await?;
             Ok(())
         }).await;
    }

     #[tokio::test]
    async fn test_imap_append_to_non_existent_folder() {
        let non_existent_folder = unique_test_folder_name("append_nonexistent");
        let folder_clone = non_existent_folder.clone();

         run_test_with_cleanup("test_imap_append_to_non_existent_folder", move |mut client| async move {
             // Ensure folder does not exist
             client.delete_folder(&folder_clone).await.ok();

             let email_body = b"Subject: Test append nonexistent\r\n\r\nBody";
             let append_result = client.append(&folder_clone, email_body, None).await;
             println!("Attempted append to non-existent folder '{}', result: {:?}", folder_clone, append_result);
            
             // IMAP standard behavior: APPEND *should* auto-create the mailbox if it doesn't exist.
             assert!(append_result.is_ok(), "Expected append to non-existent folder to succeed (auto-create)");
             
             // Verify folder was created
             let folders = client.list_folders().await?;
             assert!(folders.iter().any(|f| f.name == folder_clone), "Folder was not auto-created by APPEND");
             println!("Verified folder {} was auto-created.", folder_clone);

            // Cleanup
             client.delete_folder(&folder_clone).await?;
             Ok(())
         }).await;
    }
} 