#[cfg(test)]
mod tests {
    use super::*;
    use rustymail::imap::{ImapClient, ImapSession, ImapError, Folder, Email, SearchCriteria}; // Adjust imports
    use rustymail::mcp_port::{McpTool, McpPortError}; 
    use serde_json::{json, Value};
    use std::sync::Arc;
    use std::collections::{HashMap, HashSet}; // Added HashSet
    use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Mutex;
    use async_trait::async_trait;

    // --- Mock Tools (Keep for basic tests) ---
    struct MockEchoTool;
    #[async_trait]
    impl McpTool for MockEchoTool {
        fn name(&self) -> &str { "test/echo" }
        fn description(&self) -> &str { "Echoes parameters" }
        async fn execute(&self, params: Value) -> Result<Value, McpPortError> { Ok(params) }
    }

    struct MockNotificationTool {
        // Use a flag to check if execute was called
        called_flag: Arc<Mutex<bool>>,
    }
    #[async_trait]
    impl McpTool for MockNotificationTool {
        fn name(&self) -> &str { "test/notification" }
        fn description(&self) -> &str { "Processes notification" }
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> { 
             *self.called_flag.lock().await = true;
             // Tools handling notifications likely don't return a meaningful result
             Ok(Value::Null) 
        }
    }

    struct MockErrorTool;
    #[async_trait]
    impl McpTool for MockErrorTool {
        fn name(&self) -> &str { "test/error" }
        fn description(&self) -> &str { "Always returns an error" }
        async fn execute(&self, _params: Value) -> Result<Value, McpPortError> {
            Err(McpPortError::ToolError("This tool always fails".to_string()))
        }
    }

    // --- Mock Imap Session (Copied & adapted from rest_test.rs for consistency) ---
    #[derive(Clone, Default)]
    struct TestMockImapSession {
        folders: Arc<Mutex<Vec<Folder>>>,
        emails: Arc<Mutex<HashMap<String, Vec<Email>>>>, 
        selected_folder: Arc<Mutex<Option<String>>>,
        fail_flags: Arc<Mutex<HashMap<String, bool>>>, 
    }

    impl TestMockImapSession {
        fn set_fail_flag(&self, op: &str, fail: bool) {
            self.fail_flags.lock().blocking_lock().insert(op.to_string(), fail);
        }
        fn should_fail(&self, op: &str) -> bool {
            self.fail_flags.lock().blocking_lock().get(op).copied().unwrap_or(false)
        }
        fn add_email(&self, folder: &str, email: Email) {
            self.emails.lock().blocking_lock().entry(folder.to_string()).or_default().push(email);
        }
         fn add_folder(&self, folder: Folder) {
            self.folders.lock().blocking_lock().push(folder);
        }
    }

    #[async_trait]
    impl ImapSession for TestMockImapSession {
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
                if self.emails.lock().await.contains_key(name) && !self.emails.lock().await.get(name).unwrap().is_empty() { 
                    return Err(ImapError::Folder("Folder not empty".into()));
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
                 Ok(imap::types::Mailbox::new(name).unwrap())
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
                 _ => Ok(vec![])
             }
         }
        async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> { 
            if self.should_fail("fetch") { return Err(ImapError::Email("Mock fetch error".into())); }
            let selected = self.selected_folder.lock().await;
            let folder_name = selected.as_deref().ok_or(ImapError::InvalidState("No folder selected for fetch".into()))?; 
            let emails_map = self.emails.lock().await;
            let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
            let fetched = emails.iter().filter(|e| uids.contains(&e.uid)).cloned().collect::<Vec<_>>();
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
                     false
                 } else { true }
             });
              if uids_found.len() != uids.len() {
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
            uid, sequence: uid,
            flags: vec![], envelope: Some(rustymail::imap::Envelope { 
                date: Some("dummy_date".to_string()), subject: Some(subject.to_string()),
                from: None, sender: None, reply_to: None, to: None, cc: None, bcc: None,
                in_reply_to: None, message_id: Some(format!("<{}@test>", uid)),
             }),
            body_structure: None, internal_date: None, size: Some(1024), headers: None, body: None,
        }
    }

    // --- Test Setup ---
    fn setup_mcp_adapter_with_mock_session() -> (McpStdioAdapter, TestMockImapSession) {
        let mock_session = TestMockImapSession::default();
        let imap_client = Arc::new(ImapClient::new(Box::new(mock_session.clone())));
        let adapter = McpStdioAdapter::new(imap_client);
        (adapter, mock_session)
    }

    async fn run_adapter_test(adapter: McpStdioAdapter, input: &str) -> String {
        let mut output = String::new();
        for line in input.lines() {
            if line.trim().is_empty() { continue; }
            let request = serde_json::from_str::<JsonRpcRequest>(line);
            let response_opt = match request {
                Ok(req) => adapter.handle_request(req).await,
                Err(e) => Some(create_jsonrpc_error_response(None, error_codes::PARSE_ERROR, &format!("Parse error: {}", e)))
            };
            if let Some(response) = response_opt {
                 output.push_str(&serde_json::to_string(&response).unwrap());
                 output.push('\n');
            }
        }
        output
    }

    // Helper to create JSON-RPC request string
    fn create_request(id: Value, method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string() + "\n"
    }

    // --- Basic MCP Tests ---
    #[tokio::test]
    async fn test_mcp_method_not_found() { 
         let (adapter, _) = setup_mcp_adapter_with_mock_session();
         let request = create_request(json!("nf"), "nonexistent/method", Value::Null);
         let output = run_adapter_test(adapter, &request).await;
         let response: Value = serde_json::from_str(&output).unwrap();
         assert_eq!(response["id"], "nf"); 
         assert!(response["result"].is_null()); 
         assert_eq!(response["error"]["code"], error_codes::METHOD_NOT_FOUND);
    }
    #[tokio::test]
    async fn test_mcp_parse_error() { 
         let (adapter, _) = setup_mcp_adapter_with_mock_session();
         let request = "{ invalid json \n";
         let output = run_adapter_test(adapter, request).await;
         let response: Value = serde_json::from_str(&output).unwrap();
         assert!(response["id"].is_null()); 
         assert!(response["result"].is_null()); 
         assert_eq!(response["error"]["code"], error_codes::PARSE_ERROR);
    }
    
    // --- IMAP Tool Tests ---
    
    #[tokio::test]
    async fn test_mcp_list_folders_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default() });
        let request = create_request(json!(10), "imap/listFolders", Value::Null);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 10);
        assert!(response["error"].is_null());
        assert_eq!(response["result"].as_array().unwrap().len(), 1);
        assert_eq!(response["result"][0]["name"], "INBOX");
    }

    #[tokio::test]
    async fn test_mcp_create_folder_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        let params = json!({ "name": "Archive" });
        let request = create_request(json!(11), "imap/createFolder", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 11);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["name"], "Archive");
        assert!(mock_session.folders.lock().await.iter().any(|f| f.name == "Archive"));
    }

     #[tokio::test]
    async fn test_mcp_create_folder_exists_error() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "Existing".into(), ..Default::default() });
        let params = json!({ "name": "Existing" });
        let request = create_request(json!(12), "imap/createFolder", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 12);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::IMAP_FOLDER_EXISTS);
    }

    #[tokio::test]
    async fn test_mcp_delete_folder_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "ToDelete".into(), ..Default::default() });
        let params = json!({ "name": "ToDelete" });
        let request = create_request(json!(13), "imap/deleteFolder", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 13);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["name"], "ToDelete");
        assert!(mock_session.folders.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_delete_folder_not_found() {
        let (adapter, _mock_session) = setup_mcp_adapter_with_mock_session();
        let params = json!({ "name": "NotFound" });
        let request = create_request(json!(14), "imap/deleteFolder", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 14);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::IMAP_FOLDER_NOT_FOUND);
    }

     #[tokio::test]
    async fn test_mcp_rename_folder_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "OldName".into(), ..Default::default() });
        let params = json!({ "from": "OldName", "to": "NewName" });
        let request = create_request(json!(15), "imap/renameFolder", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 15);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["new_name"], "NewName");
        assert_eq!(mock_session.folders.lock().await[0].name, "NewName");
    }

    #[tokio::test]
    async fn test_mcp_search_emails_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default() });
        mock_session.add_email("INBOX", create_dummy_email(1, "Test"));
        mock_session.add_email("INBOX", create_dummy_email(2, "Another"));
        let params = json!({ "folder": "INBOX", "criteria": "all" });
        let request = create_request(json!(16), "imap/searchEmails", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 16);
        assert!(response["error"].is_null());
        let uids = response["result"]["uids"].as_array().unwrap();
        assert_eq!(uids.len(), 2);
        assert!(uids.contains(&json!(1)));
        assert!(uids.contains(&json!(2)));
    }

     #[tokio::test]
    async fn test_mcp_fetch_emails_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "INBOX".into(), ..Default::default() });
        mock_session.add_email("INBOX", create_dummy_email(1, "Test1"));
        mock_session.add_email("INBOX", create_dummy_email(3, "Test3"));
        let params = json!({ "folder": "INBOX", "uids": [1, 3] }); // Select folder explicitly here
        let request = create_request(json!(17), "imap/fetchEmails", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 17);
        assert!(response["error"].is_null());
        let emails = response["result"].as_array().unwrap();
        assert_eq!(emails.len(), 2);
        assert!(emails.iter().any(|e| e["uid"] == 1));
        assert!(emails.iter().any(|e| e["uid"] == 3));
    }

    #[tokio::test]
    async fn test_mcp_move_email_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { name: "Source".into(), ..Default::default() });
        mock_session.add_folder(Folder { name: "Dest".into(), ..Default::default() });
        mock_session.add_email("Source", create_dummy_email(5, "ToMove"));
        let params = json!({ "source_folder": "Source", "uids": [5], "destination_folder": "Dest" });
        let request = create_request(json!(18), "imap/moveEmail", params);
        let output = run_adapter_test(adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 18);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["destination"], "Dest");
        assert!(mock_session.emails.lock().await.get("Source").unwrap().is_empty());
        assert_eq!(mock_session.emails.lock().await.get("Dest").unwrap().len(), 1);
        assert_eq!(mock_session.emails.lock().await.get("Dest").unwrap()[0].uid, 5);
    }

} 