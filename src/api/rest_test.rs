#[cfg(test)]
mod tests {
    use actix_web::{test, web, App, http::StatusCode};
    use std::sync::Arc;
    use std::collections::{HashMap, HashSet};
    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use serde_json::{json, Value};

    use rustymail::prelude::*;
    use crate::api::rest::{configure_routes, AppState}; // Removed RestConfig as it's not needed for test setup

    // --- Mock Imap Session (Enhanced) --- 
    #[derive(Clone, Default)]
    struct MockImapSession {
        folders: Arc<Mutex<Vec<Folder>>>,
        emails: Arc<Mutex<HashMap<String, Vec<Email>>>>, // Store emails per folder
        selected_folder: Arc<Mutex<Option<String>>>, // Track selected folder
        fail_flags: Arc<Mutex<HashMap<String, bool>>>, // Flags to trigger failures
    }

    impl MockImapSession {
        fn set_fail_flag(&self, op: &str, fail: bool) {
            self.fail_flags.lock().blocking_lock().insert(op.to_string(), fail);
        }
        fn should_fail(&self, op: &str) -> bool {
            self.fail_flags.lock().blocking_lock().get(op).copied().unwrap_or(false)
        }
        // Helper to add emails for testing
        fn add_email(&self, folder: &str, email: Email) {
            self.emails.lock().blocking_lock().entry(folder.to_string()).or_default().push(email);
        }
        // Helper to add folders for testing
         fn add_folder(&self, folder: Folder) {
            self.folders.lock().blocking_lock().push(folder);
        }
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> { 
            if self.should_fail("list") { return Err(ImapError::Connection("Mock list error".into())); }
            Ok(self.folders.lock().await.clone())
         }
        async fn create_folder(&self, name: &str) -> Result<(), ImapError> { 
            if self.should_fail("create") { return Err(ImapError::Folder("Mock create error".into())); }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == name) { return Err(ImapError::Folder("Folder already exists".into())); }
            folders.push(Folder { name: name.to_string(), delimiter: Some("/".to_string()), attributes: HashSet::new() });
            Ok(())
        }
        async fn delete_folder(&self, name: &str) -> Result<(), ImapError> { 
             if self.should_fail("delete") { return Err(ImapError::Folder("Mock delete error".into())); }
             let mut folders = self.folders.lock().await;
             if let Some(pos) = folders.iter().position(|f| f.name == name) {
                 // Simulate non-empty folder error if needed
                if self.emails.lock().await.contains_key(name) && !self.emails.lock().await.get(name).unwrap().is_empty() { 
                    return Err(ImapError::Folder("Cannot delete non-empty folder".into()));
                 }
                folders.remove(pos);
                Ok(())
            } else {
                Err(ImapError::Folder("Folder not found".into()))
            }
         }
        async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> { 
            if self.should_fail("rename") { return Err(ImapError::Folder("Mock rename error".into())); }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == to) { return Err(ImapError::Folder("Target folder exists".into())); }
            if let Some(folder) = folders.iter_mut().find(|f| f.name == from) {
                folder.name = to.to_string();
                if let Some(emails) = self.emails.lock().await.remove(from) {
                     self.emails.lock().await.insert(to.to_string(), emails);
                }
                Ok(())
            } else {
                Err(ImapError::Folder("Source folder not found".into()))
            }
         }
        async fn select_folder(&self, name: &str) -> Result<imap::types::Mailbox, ImapError> { 
             if self.should_fail("select") { return Err(ImapError::Folder("Mock select error".into())); }
             if self.folders.lock().await.iter().any(|f| f.name == name) {
                 *self.selected_folder.lock().await = Some(name.to_string());
                 Ok(imap::types::Mailbox::new(name).unwrap()) // Return dummy mailbox info
             } else {
                 Err(ImapError::Folder("Folder not found".into()))
             }
         }
        async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> { 
             if self.should_fail("search") { return Err(ImapError::Email("Mock search error".into())); }
             let selected = self.selected_folder.lock().await;
             let folder_name = selected.as_deref().ok_or(ImapError::InvalidState("No folder selected for search".into()))?;
             let emails_map = self.emails.lock().await;
             let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
             match criteria {
                 SearchCriteria::Uid(uid_str) => {
                     Ok(uid_str.split(',').filter_map(|s| s.parse().ok()).filter(|uid| emails.iter().any(|e| e.uid == *uid)).collect())
                 }
                 SearchCriteria::All => Ok(emails.iter().map(|e| e.uid).collect()),
                 SearchCriteria::Subject(s_match) => Ok(emails.iter().filter(|e| e.envelope.as_ref().map_or(false, |env| env.subject.as_deref().unwrap_or("") == s_match)).map(|e| e.uid).collect()),
                 // Add other criteria mocks as needed
                 _ => Ok(vec![])
             }
         }
        async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> { 
            if self.should_fail("fetch") { return Err(ImapError::Email("Mock fetch error".into())); }
            let selected = self.selected_folder.lock().await;
            // Fetch should work even without selecting in some cases, but mock assumes select happens implicitly if needed
             let folder_name = selected.as_deref().unwrap_or("INBOX"); // Assume INBOX if not selected? Risky.
            let emails_map = self.emails.lock().await;
            let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
            let fetched = emails.iter().filter(|e| uids.contains(&e.uid)).cloned().collect::<Vec<_>>();
            // Simulate fetch error if some UIDs not found?
            // if fetched.len() != uids.len() { return Err(ImapError::Email("Some UIDs not found".into())) }
            Ok(fetched)
         }
        async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> { 
             if self.should_fail("move") { return Err(ImapError::Email("Mock move error".into())); }
             if !self.folders.lock().await.iter().any(|f| f.name == destination_folder) {
                 return Err(ImapError::Folder("Destination folder not found".into()));
             }
            let selected = self.selected_folder.lock().await;
            let source_folder = selected.as_deref().ok_or(ImapError::InvalidState("No source folder selected for move".into()))?;
             let mut emails_map = self.emails.lock().await;
             let source_emails = emails_map.entry(source_folder.to_string()).or_default();
             let mut moved_emails = Vec::new();
             let mut uids_found = HashSet::new();
             source_emails.retain(|e| {
                 if uids.contains(&e.uid) {
                     moved_emails.push(e.clone());
                     uids_found.insert(e.uid);
                     false // Remove from source
                 } else {
                     true // Keep in source
                 }
             });
              if uids_found.len() != uids.len() {
                  // Put emails back if some weren't found (atomic failure simulation)
                  source_emails.extend(moved_emails);
                  return Err(ImapError::Email("One or more source UIDs not found".into()));
              }
             emails_map.entry(destination_folder.to_string()).or_default().extend(moved_emails);
             Ok(())
         }
        async fn logout(&self) -> Result<(), ImapError> { Ok(()) }
    }

    fn create_dummy_email(uid: u32, subject: &str) -> Email {
        Email {
            uid,
            sequence: uid, // Often same as UID initially
            flags: vec![],
            envelope: Some(rustymail::imap::Envelope {
                date: Some("dummy_date".to_string()),
                subject: Some(subject.to_string()),
                from: None, sender: None, reply_to: None, to: None, cc: None, bcc: None,
                in_reply_to: None, message_id: Some(format!("<{}@test>", uid)),
            }),
            body_structure: None, internal_date: None, size: Some(1024), headers: None, body: None,
        }
    }

    // --- Test Setup ---
    async fn setup_test_app(mock_session: MockImapSession) -> impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse> {
        let imap_client = Arc::new(ImapClient::new(Box::new(mock_session)));
        let app_state = web::Data::new(AppState { imap_client });
        test::init_service( App::new().app_data(app_state).configure(configure_routes) ).await
    }

    // --- Folder Tests (Existing + Additions) --- 

    #[actix_web::test]
    async fn test_health_check() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
    #[actix_web::test]
    async fn test_list_folders_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: Vec<Folder> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 1); assert_eq!(body[0].name, "INBOX");
    }
    #[actix_web::test]
    async fn test_list_folders_error() {
        let mock_session = MockImapSession::default();
        mock_session.set_fail_flag("list", true);
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    #[actix_web::test]
    async fn test_create_folder_success() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app(mock_session.clone()).await;
        let req = test::TestRequest::post().uri("/api/v1/folders").set_json(&json!({ "name": "Sent" })).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(mock_session.folders.lock().await.iter().any(|f| f.name == "Sent"));
    }
    #[actix_web::test]
    async fn test_create_folder_conflict() {
         let mock_session = MockImapSession::default();
         mock_session.add_folder(Folder { name: "Sent".into(), ..Default::default()});
         let app = setup_test_app(mock_session).await;
         let req = test::TestRequest::post().uri("/api/v1/folders").set_json(&json!({ "name": "Sent" })).to_request();
         let resp = test::call_service(&app, req).await;
         assert_eq!(resp.status(), StatusCode::CONFLICT);
    }
    #[actix_web::test]
    async fn test_create_folder_bad_request() {
         let mock_session = MockImapSession::default();
         let app = setup_test_app(mock_session).await;
         let req = test::TestRequest::post().uri("/api/v1/folders").set_json(&json!({ "name": "" })).to_request();
         let resp = test::call_service(&app, req).await;
         assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    #[actix_web::test]
    async fn test_delete_folder_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "Trash".into(), ..Default::default()});
        let app = setup_test_app(mock_session.clone()).await;
        let req = test::TestRequest::delete().uri("/api/v1/folders/Trash").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(mock_session.folders.lock().await.is_empty());
    }
    #[actix_web::test]
    async fn test_delete_folder_not_found() {
         let mock_session = MockImapSession::default();
         let app = setup_test_app(mock_session).await;
         let req = test::TestRequest::delete().uri("/api/v1/folders/NotFound").to_request();
         let resp = test::call_service(&app, req).await;
         assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    #[actix_web::test]
    async fn test_delete_folder_not_empty() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "NotEmpty".into(), ..Default::default()});
        mock_session.add_email("NotEmpty", create_dummy_email(1, "Subject")); // Make it non-empty
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::delete().uri("/api/v1/folders/NotEmpty").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR); // Or map to CONFLICT/BAD_REQUEST if preferred
    }
    #[actix_web::test]
    async fn test_rename_folder_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "Old".into(), ..Default::default()});
        let app = setup_test_app(mock_session.clone()).await;
        let req = test::TestRequest::put().uri("/api/v1/folders/Old/rename").set_json(&json!({ "new_name": "New" })).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(mock_session.folders.lock().await[0].name, "New");
    }
    #[actix_web::test]
    async fn test_rename_folder_source_not_found() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::put().uri("/api/v1/folders/NotFound/rename").set_json(&json!({ "new_name": "New" })).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
     #[actix_web::test]
    async fn test_rename_folder_target_exists() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "Old".into(), ..Default::default()});
        mock_session.add_folder(Folder { name: "Existing".into(), ..Default::default()});
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::put().uri("/api/v1/folders/Old/rename").set_json(&json!({ "new_name": "Existing" })).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    // --- Email Tests --- 

    #[actix_web::test]
    async fn test_search_emails_all_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "Subj1"));
        mock_session.add_email("INBOX", create_dummy_email(5, "Subj5"));
        let app = setup_test_app(mock_session).await;
        // Note: requires folder name in path
        let req = test::TestRequest::get().uri("/api/v1/folders/INBOX/emails?criteria=All").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["uids"].as_array().unwrap().len(), 2);
        assert!(body["uids"].as_array().unwrap().contains(&json!(1)));
        assert!(body["uids"].as_array().unwrap().contains(&json!(5)));
    }

     #[actix_web::test]
    async fn test_search_emails_by_subject() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "FindMe"));
        mock_session.add_email("INBOX", create_dummy_email(5, "IgnoreMe"));
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/folders/INBOX/emails?criteria=Subject&value=FindMe").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["uids"].as_array().unwrap().len(), 1);
        assert_eq!(body["uids"].as_array().unwrap()[0], json!(1));
    }

    #[actix_web::test]
    async fn test_search_emails_folder_not_found() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/folders/NotFound/emails?criteria=All").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

     #[actix_web::test]
    async fn test_fetch_emails_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "Subj1"));
        mock_session.add_email("INBOX", create_dummy_email(3, "Subj3"));
        mock_session.add_email("INBOX", create_dummy_email(5, "Subj5"));
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/emails?uids=1,5").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Vec<Email> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);
        assert!(body.iter().any(|e| e.uid == 1));
        assert!(body.iter().any(|e| e.uid == 5));
    }

    #[actix_web::test]
    async fn test_fetch_emails_some_not_found() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "Subj1"));
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::get().uri("/api/v1/emails?uids=1,99").to_request(); // 99 doesn't exist
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK); // Fetch might partially succeed
        let body: Vec<Email> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 1); // Only UID 1 is returned
        assert_eq!(body[0].uid, 1);
    }

    #[actix_web::test]
    async fn test_fetch_emails_bad_request() {
         let mock_session = MockImapSession::default();
         let app = setup_test_app(mock_session).await;
         let req = test::TestRequest::get().uri("/api/v1/emails?uids=1,bad,3").to_request(); // Invalid UID format
         let resp = test::call_service(&app, req).await;
         assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

     #[actix_web::test]
    async fn test_move_email_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_folder(Folder { name: "Archive".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "one"));
        mock_session.add_email("INBOX", create_dummy_email(2, "two"));
        // Select INBOX implicitly before move (required by mock)
        mock_session.select_folder("INBOX").await.unwrap(); 
        let app = setup_test_app(mock_session.clone()).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/emails/move")
            .set_json(&json!({ "uids": "1,2", "destination_folder": "Archive" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        // Verify emails moved in mock state
        assert!(mock_session.emails.lock().await.get("INBOX").unwrap().is_empty());
        assert_eq!(mock_session.emails.lock().await.get("Archive").unwrap().len(), 2);
    }

    #[actix_web::test]
    async fn test_move_email_dest_not_found() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "one"));
        mock_session.select_folder("INBOX").await.unwrap();
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/emails/move")
            .set_json(&json!({ "uids": "1", "destination_folder": "NotFound" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND); // Destination folder check fails
    }

    #[actix_web::test]
    async fn test_move_email_uid_not_found() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default()});
        mock_session.add_folder(Folder { name: "Archive".into(), ..Default::default()});
        mock_session.add_email("INBOX", create_dummy_email(1, "one"));
        mock_session.select_folder("INBOX").await.unwrap();
        let app = setup_test_app(mock_session).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/emails/move")
            .set_json(&json!({ "uids": "1,99", "destination_folder": "Archive" })) // UID 99 not found
            .to_request();
        // The mock ImapSession move fails atomically if one UID is bad
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR); // Maps from ImapError::Email
        // Verify email 1 was NOT moved
         assert_eq!(mock_session.emails.lock().await.get("INBOX").unwrap().len(), 1);
         assert!(mock_session.emails.lock().await.get("Archive").unwrap().is_empty());
    }
} 