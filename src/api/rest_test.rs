#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use imap_types::envelope::Envelope;
    use imap_types::mailbox::Mailbox;
    use actix_web::{test, web, App, http::StatusCode, dev::ServiceResponse, HttpRequest};
    use std::sync::Arc;
    use std::collections::{HashMap, HashSet};
    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use serde_json::{json, Value};
    use actix_web::dev::Service;
    use actix_web::http::{header, StatusCode};
    use actix_web::{test, web, App, HttpRequest};
    use imap_types::envelope::{Envelope, Address};
    use imap_types::core::NString;
    use bytes::Bytes;

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
            futures::executor::block_on(async { 
                 self.fail_flags.lock().await.insert(op.to_string(), fail);
             });
        }
        fn should_fail(&self, op: &str) -> bool {
            futures::executor::block_on(async { 
                self.fail_flags.lock().await.get(op).copied().unwrap_or(false)
            })
        }
        // Helper to add emails for testing
        fn add_email(&self, folder: &str, email: Email) {
            futures::executor::block_on(async { 
                 self.emails.lock().await.entry(folder.to_string()).or_default().push(email);
             });
        }
        // Helper to add folders for testing
         fn add_folder(&self, folder: Folder) {
            futures::executor::block_on(async { 
                 self.folders.lock().await.push(folder);
             });
        }
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> { 
            if self.should_fail("list") { return Err(ImapError::Connection("Mock list connection error".into())); }
            Ok(self.folders.lock().await.clone())
         }
        async fn create_folder(&self, name: &str) -> Result<(), ImapError> { 
            if self.should_fail("create") { return Err(ImapError::Operation("Mock create operation error".into())); }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == name) { return Err(ImapError::BadResponse("Folder already exists".into())); }
            folders.push(Folder { name: name.to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
            Ok(())
        }
        async fn delete_folder(&self, name: &str) -> Result<(), ImapError> { 
             if self.should_fail("delete") { return Err(ImapError::Operation("Mock delete operation error".into())); }
             let mut folders = self.folders.lock().await;
             if let Some(pos) = folders.iter().position(|f| f.name == name) {
                 if self.emails.lock().await.contains_key(name) && !self.emails.lock().await.get(name).unwrap().is_empty() { 
                    return Err(ImapError::BadResponse("Folder not empty".into()));
                 }
                folders.remove(pos);
                Ok(())
            } else {
                Err(ImapError::BadResponse("Folder not found".into()))
            }
         }
        async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> { 
            if self.should_fail("rename") { return Err(ImapError::Operation("Mock rename operation error".into())); }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == to) { return Err(ImapError::BadResponse("Target folder exists".into())); }
            if let Some(folder) = folders.iter_mut().find(|f| f.name == from) {
                folder.name = to.to_string();
                if let Some(emails) = self.emails.lock().await.remove(from) {
                     self.emails.lock().await.insert(to.to_string(), emails);
                }
                Ok(())
            } else {
                Err(ImapError::BadResponse("Source folder not found".into()))
            }
         }
        async fn select_folder(&self, name: &str) -> Result<Mailbox, ImapError> { 
             if self.should_fail("select") { return Err(ImapError::Operation("Mock select operation error".into())); }
             if self.folders.lock().await.iter().any(|f| f.name == name) {
                 *self.selected_folder.lock().await = Some(name.to_string());
                 if name.eq_ignore_ascii_case("INBOX") {
                     Ok(Mailbox::Inbox)
                 } else {
                     Err(ImapError::Operation("Mock select non-INBOX not fully implemented".into()))
                 }
             } else {
                 Err(ImapError::BadResponse("Folder not found".into()))
             }
         }
        async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> { 
             if self.should_fail("search") { return Err(ImapError::Operation("Mock search operation error".into())); }
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
            if self.should_fail("fetch") { return Err(ImapError::Operation("Mock fetch operation error".into())); }
            let selected = self.selected_folder.lock().await;
            let folder_name = selected.as_deref().ok_or(ImapError::InvalidState("No folder selected for fetch".into()))?;
            let emails_map = self.emails.lock().await;
            let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
            let fetched = emails.iter().filter(|e| uids.contains(&e.uid)).cloned().collect::<Vec<_>>();
            // Simulate fetch error if some UIDs not found?
            // if fetched.len() != uids.len() { return Err(ImapError::Email("Some UIDs not found".into())) }
            Ok(fetched)
         }
        async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> { 
             if self.should_fail("move") { return Err(ImapError::Operation("Mock move operation error".into())); }
             if !self.folders.lock().await.iter().any(|f| f.name == destination_folder) {
                 return Err(ImapError::BadResponse("Destination folder not found".into()));
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
                  return Err(ImapError::BadResponse("One or more source UIDs not found".into()));
              }
             emails_map.entry(destination_folder.to_string()).or_default().extend(moved_emails);
             Ok(())
         }
        async fn logout(&self) -> Result<(), ImapError> { Ok(()) }
    }

    fn create_dummy_email(uid: u32, subject: &str) -> Email {
        Email {
            uid,
            flags: vec![],
            envelope: Some(Envelope {
                date: Some(Bytes::from("dummy_date")),
                subject: Some(Bytes::from(subject.as_bytes())),
                from: None,
                sender: None,
                reply_to: None,
                to: None,
                cc: None,
                bcc: None,
                in_reply_to: None,
                message_id: NString::try_from(Bytes::from(format!("<{}>", uid))).ok(),
            }),
            size: Some(1024),
        }
    }

    // --- Test Setup ---
    async fn setup_test_app_service(mock_session: MockImapSession) -> impl Service<HttpRequest, Response = ServiceResponse, Error = actix_web::Error> {
        let imap_session_arc: Arc<dyn ImapSession> = Arc::new(mock_session);
        let imap_client = Arc::new(ImapClient::new(imap_session_arc));
        let app_state = web::Data::new(AppState { imap_client });
        // Return the initialized service directly
        test::init_service( App::new().app_data(app_state).configure(configure_routes) ).await
    }

    // --- Folder Tests (Existing + Additions) --- 

    #[actix_web::test]
    async fn test_health_check() {
        let mock_session = MockImapSession::default();
        // Get the initialized service
        let app = setup_test_app_service(mock_session).await;

        // Create request using TestRequest
        let req = test::TestRequest::get().uri("/api/v1/health").to_request();
        
        // Send the request using Service::call
        let resp = app.call(req).await.unwrap(); // Use Service::call
        
        // Check the status code on the ServiceResponse
        assert_eq!(resp.status(), StatusCode::OK);

        // Read the body from the ServiceResponse
        let body = test::read_body(resp).await;
        assert_eq!(body, Bytes::from_static(b"OK"));
    }

    #[actix_web::test]
    async fn test_list_folders_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_folder(Folder { name: "Sent".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        let app = setup_test_app_service(mock_session).await; // Use new setup fn
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Vec<Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);
        assert_eq!(body[0]["name"], "INBOX");
        assert_eq!(body[1]["name"], "Sent");
    }

    #[actix_web::test]
    async fn test_list_folders_error() {
        let mock_session = MockImapSession::default();
        mock_session.set_fail_flag("list", true);
        let app = setup_test_app_service(mock_session).await; // Use new setup fn
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "IMAP Connection Error");
        assert!(body["message"].as_str().unwrap().contains("Mock list connection error"));
    }

    #[actix_web::test]
    async fn test_create_folder_success() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app_service(mock_session.clone()).await; // Use new setup fn & clone mock
        let req = test::TestRequest::post().uri("/api/v1/folders").set_json(&json!({ "name": "Archive" })).to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::CREATED);
        // Verify folder was added in mock
        assert!(futures::executor::block_on(async { mock_session.folders.lock().await.iter().any(|f| f.name == "Archive") }));
    }

    #[actix_web::test]
    async fn test_create_folder_conflict() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "Existing".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&json!({ "name": "Existing" }))
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_create_folder_error() {
        let mock_session = MockImapSession::default();
        mock_session.set_fail_flag("create", true);
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&json!({ "name": "ErrorCase" }))
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[actix_web::test]
    async fn test_delete_folder_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "ToDelete".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        let app = setup_test_app_service(mock_session.clone()).await; 
        let req = test::TestRequest::delete()
            .uri("/api/v1/folders/ToDelete")
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(futures::executor::block_on(async { !mock_session.folders.lock().await.iter().any(|f| f.name == "ToDelete") }));
    }

    #[actix_web::test]
    async fn test_delete_folder_not_found() {
        let mock_session = MockImapSession::default();
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::delete()
            .uri("/api/v1/folders/NotFound")
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_delete_folder_not_empty() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "NotEmpty".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_email("NotEmpty", create_dummy_email(1, "test"));
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::delete()
            .uri("/api/v1/folders/NotEmpty")
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::CONFLICT); // Expect Conflict for non-empty deletion attempt
    }

    #[actix_web::test]
    async fn test_delete_folder_error() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "ErrorDelete".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.set_fail_flag("delete", true);
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::delete()
            .uri("/api/v1/folders/ErrorDelete")
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[actix_web::test]
    async fn test_rename_folder_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "OldName".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        let app = setup_test_app_service(mock_session.clone()).await;
        let req = test::TestRequest::put()
            .uri("/api/v1/folders/OldName/rename")
            .set_json(&json!({ "new_name": "NewName" }))
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(futures::executor::block_on(async { !mock_session.folders.lock().await.iter().any(|f| f.name == "OldName") }));
        assert!(futures::executor::block_on(async { mock_session.folders.lock().await.iter().any(|f| f.name == "NewName") }));
    }

    // --- Email Tests --- 

    #[actix_web::test]
    async fn test_search_emails_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_email("INBOX", create_dummy_email(1, "Subject 1"));
        mock_session.add_email("INBOX", create_dummy_email(2, "Subject 2"));
        // Mock selecting the folder
        futures::executor::block_on(async { *mock_session.selected_folder.lock().await = Some("INBOX".to_string()); });
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::get()
            .uri("/api/v1/folders/INBOX/emails/search?q=SUBJECT%20Subject%201") // URL encoded subject search
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert!(body["uids"].is_array());
        assert_eq!(body["uids"].as_array().unwrap().len(), 1);
        assert_eq!(body["uids"][0].as_u64().unwrap(), 1);
    }

    #[actix_web::test]
    async fn test_fetch_emails_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_email("INBOX", create_dummy_email(1, "Subject 1"));
        mock_session.add_email("INBOX", create_dummy_email(2, "Subject 2"));
        // Mock selecting the folder
        futures::executor::block_on(async { *mock_session.selected_folder.lock().await = Some("INBOX".to_string()); });
        let app = setup_test_app_service(mock_session).await;
        let req = test::TestRequest::get()
            .uri("/api/v1/folders/INBOX/emails/fetch?uids=1,2")
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Vec<Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);
        assert_eq!(body[0]["uid"].as_u64().unwrap(), 1);
        assert_eq!(body[1]["uid"].as_u64().unwrap(), 2);
        assert_eq!(body[0]["envelope"]["subject"].as_str().unwrap(), "Subject 1");
    }

    #[actix_web::test]
    async fn test_move_email_success() {
        let mock_session = MockImapSession::default();
        mock_session.add_folder(Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_folder(Folder { name: "Archive".to_string(), delimiter: Some("/".to_string()), attributes: vec![] });
        mock_session.add_email("INBOX", create_dummy_email(1, "To Move"));
        // Mock selecting the folder
        futures::executor::block_on(async { *mock_session.selected_folder.lock().await = Some("INBOX".to_string()); });
        let app = setup_test_app_service(mock_session.clone()).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/folders/INBOX/emails/move")
            .set_json(&json!({ "destination_folder": "Archive", "uids": [1] }))
            .to_request();
        let resp = app.call(req).await.unwrap(); // Use Service::call
        assert_eq!(resp.status(), StatusCode::OK);
        // Verify move in mock
        assert!(futures::executor::block_on(async { mock_session.emails.lock().await.get("INBOX").unwrap().is_empty() }));
        assert_eq!(futures::executor::block_on(async { mock_session.emails.lock().await.get("Archive").unwrap().len() }), 1);
        assert_eq!(futures::executor::block_on(async { mock_session.emails.lock().await.get("Archive").unwrap()[0].uid }), 1);
    }
} 