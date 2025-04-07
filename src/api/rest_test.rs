#[cfg(test)]
mod tests {
    use actix_web::{test, web, App, http::StatusCode};
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tokio::sync::Mutex;
    use serde_json::json;
    use urlencoding;

    use crate::api::rest::{configure_rest_service, AppState};
    use crate::imap::client::ImapClient;
    use crate::imap::session::ImapSession;
    use crate::imap::error::ImapError;
    use crate::imap::types::{Folder, Email, MailboxInfo, SearchCriteria};
    use async_imap::types::Mailbox as AsyncMailbox; // Keep for mock definition
    use async_trait::async_trait;

    // --- Mock IMAP Session (Copied & adapted from client_test.rs for simplicity) ---
    #[derive(Debug, Default)]
    struct MockCallTracker {
        list_folders_called: AtomicBool,
        create_folder_called: AtomicBool,
        delete_folder_called: AtomicBool,
        rename_folder_called: AtomicBool,
        select_folder_called: AtomicBool,
        search_emails_called: AtomicBool,
        fetch_emails_called: AtomicBool,
        move_email_called: AtomicBool,
        logout_called: AtomicBool,
    }

    #[derive(Debug, Clone)] // Ensure MockImapSession is Clone for AppState
    struct MockImapSession {
        tracker: Arc<MockCallTracker>,
        list_folders_result: Result<Vec<Folder>, ImapError>,
        select_folder_result: Result<MailboxInfo, ImapError>,
        search_emails_result: Result<Vec<u32>, ImapError>,
        fetch_emails_result: Result<Vec<Email>, ImapError>,
        create_result: Result<(), ImapError>,
        delete_result: Result<(), ImapError>,
        rename_result: Result<(), ImapError>,
        move_result: Result<(), ImapError>,
        logout_result: Result<(), ImapError>,
    }

    impl MockImapSession {
        fn default_ok() -> Self {
            Self {
                tracker: Arc::new(MockCallTracker::default()),
                list_folders_result: Ok(vec![
                    Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()) },
                    Folder { name: "Sent".to_string(), delimiter: Some("/".to_string()) },
                ]),
                select_folder_result: Ok(MailboxInfo {
                    flags: vec!["\\Seen".to_string()], exists: 10, recent: 1,
                    unseen: Some(5), permanent_flags: vec!["\*"].iter().map(|s| s.to_string()).collect(),
                    uid_next: Some(101), uid_validity: Some(12345),
                 }),
                search_emails_result: Ok(vec![1, 2, 3]),
                fetch_emails_result: Ok(vec![Email { uid: 1, flags: vec![], size: Some(100), envelope: None }]),
                create_result: Ok(()),
                delete_result: Ok(()),
                rename_result: Ok(()),
                move_result: Ok(()),
                logout_result: Ok(()),
            }
        }
         fn set_select_result(mut self, res: Result<MailboxInfo, ImapError>) -> Self {
            self.select_folder_result = res;
            self
        }
         fn set_list_folders_result(mut self, res: Result<Vec<Folder>, ImapError>) -> Self {
            self.list_folders_result = res;
            self
        }
        fn set_create_result(mut self, res: Result<(), ImapError>) -> Self {
            self.create_result = res;
            self
        }
        fn set_delete_result(mut self, res: Result<(), ImapError>) -> Self {
            self.delete_result = res;
            self
        }
        fn set_rename_result(mut self, res: Result<(), ImapError>) -> Self {
            self.rename_result = res;
            self
        }
        fn set_search_emails_result(mut self, res: Result<Vec<u32>, ImapError>) -> Self {
            self.search_emails_result = res;
            self
        }
        fn set_fetch_emails_result(mut self, res: Result<Vec<Email>, ImapError>) -> Self {
            self.fetch_emails_result = res;
            self
        }
        fn set_move_result(mut self, res: Result<(), ImapError>) -> Self {
            self.move_result = res;
            self
        }
        fn set_logout_result(mut self, res: Result<(), ImapError>) -> Self {
            self.logout_result = res;
            self
        }
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
            self.tracker.list_folders_called.store(true, Ordering::SeqCst);
            self.list_folders_result.clone()
        }
        async fn create_folder(&self, _name: &str) -> Result<(), ImapError> {
            self.tracker.create_folder_called.store(true, Ordering::SeqCst);
            self.create_result.clone()
        }
        async fn delete_folder(&self, _name: &str) -> Result<(), ImapError> {
            self.tracker.delete_folder_called.store(true, Ordering::SeqCst);
            self.delete_result.clone()
        }
        async fn rename_folder(&self, _from: &str, _to: &str) -> Result<(), ImapError> {
            self.tracker.rename_folder_called.store(true, Ordering::SeqCst);
            self.rename_result.clone()
        }
        async fn select_folder(&self, _name: &str) -> Result<MailboxInfo, ImapError> {
            self.tracker.select_folder_called.store(true, Ordering::SeqCst);
            self.select_folder_result.clone()
        }
        async fn search_emails(&self, _criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
            self.tracker.search_emails_called.store(true, Ordering::SeqCst);
            self.search_emails_result.clone()
        }
        async fn fetch_emails(&self, _uids: Vec<u32>) -> Result<Vec<Email>, ImapError> {
            self.tracker.fetch_emails_called.store(true, Ordering::SeqCst);
            if _uids.is_empty() { return Ok(Vec::new()); }
            self.fetch_emails_result.clone()
        }
        async fn move_email(&self, _uids: Vec<u32>, _dest: &str) -> Result<(), ImapError> {
            self.tracker.move_email_called.store(true, Ordering::SeqCst);
            if _uids.is_empty() { return Ok(()); }
            self.move_result.clone()
        }
        async fn logout(&self) -> Result<(), ImapError> {
            self.tracker.logout_called.store(true, Ordering::SeqCst);
            self.logout_result.clone()
        }
    }

    // --- Test Setup --- 
    async fn setup_test_app() -> (impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse>, Arc<MockCallTracker>) {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();

        // Create ImapClient backed by the mock session
        let mock_imap_client = Arc::new(ImapClient::new_with_session(Arc::new(Mutex::new(mock_session))));

        // Create AppState
        let app_state = AppState { imap_client: mock_imap_client };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state.clone()))
                .configure(configure_rest_service)
        ).await;
        (app, tracker)
    }

    // --- Test Cases --- 

    #[actix_web::test]
    async fn test_health_check() {
        let (mut app, _) = setup_test_app().await;
        let req = test::TestRequest::get().uri("/api/v1/health").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body, json!({ "status": "OK" }));
    }

    #[actix_web::test]
    async fn test_list_folders_route() {
        let (mut app, tracker) = setup_test_app().await;
        let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
        let resp = test::call_service(&mut app, req).await;
        
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.list_folders_called.load(Ordering::SeqCst));

        let folders: Vec<Folder> = test::read_body_json(resp).await;
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "INBOX");
    }

    #[actix_web::test]
    async fn test_create_folder_route() {
        let (mut app, tracker) = setup_test_app().await;
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&json!({ "name": "New Folder" }))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(tracker.create_folder_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["message"], "Folder 'New Folder' created");
    }

    #[actix_web::test]
    async fn test_select_folder_route() {
        let (mut app, tracker) = setup_test_app().await;
        let folder_name = "INBOX";
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/folders/{}/select", folder_name))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.select_folder_called.load(Ordering::SeqCst));

        // Check the returned MailboxInfo
        let mailbox_info: MailboxInfo = test::read_body_json(resp).await;
        assert_eq!(mailbox_info.exists, 10);
        assert_eq!(mailbox_info.uid_validity, Some(12345));
    }

    #[actix_web::test]
    async fn test_delete_folder_route() {
        let (mut app, tracker) = setup_test_app().await;
        let folder_name = "ToDelete";
        let encoded_name = urlencoding::encode(folder_name);
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/folders/{}", encoded_name))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.delete_folder_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].as_str().unwrap().contains(folder_name));
    }

    #[actix_web::test]
    async fn test_rename_folder_route() {
        let (mut app, tracker) = setup_test_app().await;
        let from_name = "OldName";
        let encoded_from = urlencoding::encode(from_name);
        let to_name = "New Name";
        let req = test::TestRequest::put()
            .uri(&format!("/api/v1/folders/{}", encoded_from))
            .set_json(&json!({ "to_name": to_name }))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.rename_folder_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].as_str().unwrap().contains(from_name));
        assert!(body["message"].as_str().unwrap().contains(to_name));
    }

    #[actix_web::test]
    async fn test_search_emails_route() {
        let (mut app, tracker) = setup_test_app().await;
        // Example: Search for subject "Test"
        let req = test::TestRequest::get()
            .uri("/api/v1/emails/search?subject=Test") // Assumes folder already selected
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.search_emails_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["uids"].as_array().unwrap(), &vec![json!(1), json!(2), json!(3)]);
    }

    #[actix_web::test]
    async fn test_fetch_emails_route() {
        let (mut app, tracker) = setup_test_app().await;
        let uids = "1,5,10";
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/emails/fetch?uids={}", uids))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.fetch_emails_called.load(Ordering::SeqCst));
        let emails: Vec<Email> = test::read_body_json(resp).await;
        // Assert based on MockImapSession::default_ok fetch_emails_result
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].uid, 1);
    }

    #[actix_web::test]
    async fn test_move_emails_route() {
        let (mut app, tracker) = setup_test_app().await;
        let uids = vec![1, 2];
        let dest_folder = "Archive";
        let req = test::TestRequest::post()
            .uri("/api/v1/emails/move")
            .set_json(&json!({ "uids": uids, "destination_folder": dest_folder }))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(tracker.move_email_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].as_str().unwrap().contains(dest_folder));
    }

    #[actix_web::test]
    async fn test_delete_folder_not_found() {
        // Setup mock to return an error simulating "not found"
        let mock_session = MockImapSession::default_ok()
            .set_delete_result(Err(ImapError::Operation("Folder does not exist".to_string())));
        let tracker = mock_session.tracker.clone();
        let mock_imap_client = Arc::new(ImapClient::new_with_session(Arc::new(Mutex::new(mock_session))));
        let app_state = AppState { imap_client: mock_imap_client };
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state.clone()))
                .configure(configure_rest_service)
        ).await;

        let folder_name = "NonExistent";
        let encoded_name = urlencoding::encode(folder_name);
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/folders/{}", encoded_name))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        // Check that the ApiError::ImapOperationFailed maps to 404 Not Found
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert!(tracker.delete_folder_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].as_str().unwrap().contains("IMAP operation failed"));
        // Note: The exact error message might differ based on the From<ImapError> impl
    }

    #[actix_web::test]
    async fn test_create_folder_bad_request() {
        let (mut app, tracker) = setup_test_app().await;
        // Send empty name
        let req = test::TestRequest::post()
            .uri("/api/v1/folders")
            .set_json(&json!({ "name": "  " })) // Empty/whitespace name
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        // Ensure the mock was *not* called for a bad request handled early
        assert!(!tracker.create_folder_called.load(Ordering::SeqCst));
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].as_str().unwrap().contains("cannot be empty"));
    }

    // TODO: Add more error case tests (e.g., invalid UIDs for fetch/move, rename conflicts)
} 