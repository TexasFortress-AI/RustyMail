#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use crate::api::mcp::McpStdioAdapter;
    use crate::api::mcp::error_codes;
    use imap_types::envelope::Envelope;
    use imap_types::mailbox::Mailbox;
    use tokio::io::{self, AsyncReadExt, AsyncWriteExt, DuplexStream};
    use tokio::sync::Mutex;
    use std::sync::Arc;
    use std::collections::{HashMap, HashSet};
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use bytes;
    use futures;
    use imap_types::envelope::{Envelope, Address};
    use imap_types::core::NString;
    use rustymail::prelude::{Email, Folder, SearchCriteria, ImapError, ImapSession};

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

    // --- Mock Imap Session (Similar to rest_test, maybe factor out?) ---
    #[derive(Clone, Default)]
    struct TestMockImapSession {
        folders: Arc<Mutex<Vec<Folder>>>,
        emails: Arc<Mutex<HashMap<String, Vec<Email>>>>, 
        selected_folder: Arc<Mutex<Option<String>>>,
        fail_flags: Arc<Mutex<HashMap<String, bool>>>, 
    }

    impl TestMockImapSession {
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
        fn add_email(&self, folder: &str, email: Email) {
            futures::executor::block_on(async { 
                 self.emails.lock().await.entry(folder.to_string()).or_default().push(email);
             });
        }
         fn add_folder(&self, folder: Folder) {
            futures::executor::block_on(async { 
                 self.folders.lock().await.push(folder);
             });
        }
    }

    #[async_trait]
    impl ImapSession for TestMockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> { 
            if self.should_fail("list") { return Err(ImapError::Connection("Mock list connection error".into())); }
            Ok(self.folders.lock().await.clone())
         }
        async fn create_folder(&self, name: &str) -> Result<(), ImapError> { 
            if self.should_fail("create") { return Err(ImapError::Operation("Mock create operation error".into())); }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == name) { return Err(ImapError::BadResponse("Folder already exists".into())); }
            folders.push(Folder { 
                name: name.to_string(), 
                delimiter: Some("/".to_string()), 
            });
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
             let folder_name = selected.as_deref().ok_or(ImapError::Operation("No folder selected".into()))?;
             match criteria {
                 SearchCriteria::Uid(ref uid_str) => {
                     let uids_to_find: HashSet<u32> = uid_str.split(',')
                         .filter_map(|s| s.trim().parse::<u32>().ok())
                         .collect();
                     let emails_map = self.emails.lock().await;
                     let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
                     Ok(emails.iter().map(|e| e.uid).filter(|uid| uids_to_find.contains(uid)).collect())
                 }
                 SearchCriteria::Subject(ref s_match) => {
                    let emails_map = self.emails.lock().await;
                    let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
                     Ok(emails.iter()
                         .filter(|e| e.envelope.as_ref().map_or(false, |env| env.subject.as_ref().map_or(false, |subj| subj.as_ref().contains(s_match))))
                         .map(|e| e.uid).collect())
                 }
                 SearchCriteria::All => Ok(emails.iter().map(|e| e.uid).collect()),
                 _ => Ok(vec![])
             }
         }
        async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> { 
            if self.should_fail("fetch") { return Err(ImapError::Operation("Mock fetch operation error".into())); }
            let selected = self.selected_folder.lock().await;
            let folder_name = selected.as_deref().ok_or(ImapError::Operation("No folder selected".into()))?;
            let emails_map = self.emails.lock().await;
            let emails = emails_map.get(folder_name).map(|v| v.as_slice()).unwrap_or(&[]);
            let fetched = emails.iter().filter(|e| uids.contains(&e.uid)).cloned().collect::<Vec<_>>();
            Ok(fetched)
         }
        async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> { 
             if self.should_fail("move") { return Err(ImapError::Operation("Mock move operation error".into())); }
             if !self.folders.lock().await.iter().any(|f| f.name == destination_folder) {
                 return Err(ImapError::BadResponse("Destination folder not found".into()));
             }
            let selected = self.selected_folder.lock().await;
            let source_folder = selected.as_deref().ok_or(ImapError::Operation("No source folder selected for move".into()))?;
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
            flags: vec!["\Seen".to_string()],
            size: Some(1024),
            envelope: Some(Envelope {
                date: NString::try_from("Wed, 17 Jul 2024 10:00:00 +0000".to_string()).unwrap(), 
                subject: NString::try_from(subject.to_string()).unwrap(),
                from: vec![],
                sender: vec![],
                reply_to: vec![],
                to: vec![],
                cc: vec![],
                bcc: vec![],
                in_reply_to: NString::try_from("".to_string()).unwrap(), 
                message_id: NString::try_from("".to_string()).unwrap(), 
            }),
        }
    }

    // --- Test Setup ---
    fn setup_mcp_adapter_with_mock_session() -> (McpStdioAdapter, TestMockImapSession) {
        let mock_session = TestMockImapSession::default();
        let imap_session_arc: Arc<dyn ImapSession> = Arc::new(mock_session.clone());
        let imap_client = Arc::new(ImapClient::new(imap_session_arc)); 
        let adapter = McpStdioAdapter::new(imap_client);
        (adapter, mock_session)
    }

    async fn run_adapter_test(adapter: &McpStdioAdapter, input: &str) -> String {
        let mut results = String::new();
        for line in input.lines() {
            if line.trim().is_empty() { continue; }
            let request = serde_json::from_str::<crate::api::mcp::JsonRpcRequest>(line);
            let response = match request {
                Ok(req) => adapter.handle_request(req).await,
                Err(e) => Some(crate::api::mcp::create_jsonrpc_error_response(None, error_codes::PARSE_ERROR, &format!("Parse error: {}", e)))
            };
            if let Some(resp) = response {
                results.push_str(&serde_json::to_string(&resp).unwrap());
                results.push('\n');
            }
        }
        results
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
         let output = run_adapter_test(&adapter, &request).await;
         let response: Value = serde_json::from_str(&output).unwrap();
         assert_eq!(response["id"], "nf"); 
         assert!(response["result"].is_null()); 
         assert_eq!(response["error"]["code"], error_codes::METHOD_NOT_FOUND);
    }
    #[tokio::test]
    async fn test_mcp_parse_error() { 
         let (adapter, _) = setup_mcp_adapter_with_mock_session();
         let request = "{ invalid json \n";
         let output = run_adapter_test(&adapter, request).await;
         let response: Value = serde_json::from_str(&output).unwrap();
         assert!(response["id"].is_null()); 
         assert!(response["result"].is_null()); 
         assert_eq!(response["error"]["code"], error_codes::PARSE_ERROR);
    }
    
    // --- IMAP Tool Tests ---
    
    #[tokio::test]
    async fn test_mcp_list_folders_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { 
            name: "INBOX".into(), 
            delimiter: Some("/".into()), 
        });
        let request = create_request(json!(10), "imap/listFolders", Value::Null);
        let output = run_adapter_test(&adapter, &request).await;
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
        let output = run_adapter_test(&adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 11);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["name"], "Archive");
        assert!(mock_session.folders.lock().await.iter().any(|f| f.name == "Archive"));
    }

     #[tokio::test]
    async fn test_mcp_create_folder_exists_error() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { 
            name: "Existing".into(), 
            delimiter: Some("/".into()), 
        });
        let params = json!({ "name": "Existing" });
        let request = create_request(json!(12), "imap/createFolder", params);
        let output = run_adapter_test(&adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 12);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::IMAP_FOLDER_EXISTS);
    }

    #[tokio::test]
    async fn test_mcp_delete_folder_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { 
            name: "ToDelete".into(), 
            delimiter: Some("/".into()), 
        });
        let params = json!({ "name": "ToDelete" });
        let request = create_request(json!(13), "imap/deleteFolder", params);
        let output = run_adapter_test(&adapter, &request).await;
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
        let output = run_adapter_test(&adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 14);
        assert!(response["result"].is_null());
        assert_eq!(response["error"]["code"], error_codes::IMAP_FOLDER_NOT_FOUND);
    }

     #[tokio::test]
    async fn test_mcp_rename_folder_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { 
            name: "OldName".into(), 
            delimiter: Some("/".into()), 
        });
        let params = json!({ "from": "OldName", "to": "NewName" });
        let request = create_request(json!(15), "imap/renameFolder", params);
        let output = run_adapter_test(&adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 15);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["new_name"], "NewName");
        assert_eq!(mock_session.folders.lock().await[0].name, "NewName");
    }

    #[tokio::test]
    async fn test_mcp_search_emails_success() {
        let (adapter, mock_session) = setup_mcp_adapter_with_mock_session();
        mock_session.add_folder(Folder { 
            name: "INBOX".into(), 
            delimiter: Some("/".into()), 
        });
        mock_session.add_email("INBOX", create_dummy_email(1, "Test"));
        mock_session.add_email("INBOX", create_dummy_email(2, "Another"));
        let params = json!({ "folder": "INBOX", "criteria": "all" });
        let request = create_request(json!(16), "imap/searchEmails", params);
        let output = run_adapter_test(&adapter, &request).await;
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
        mock_session.add_folder(Folder { 
            name: "INBOX".into(), 
            delimiter: Some("/".into()), 
        });
        mock_session.add_email("INBOX", create_dummy_email(1, "Test1"));
        mock_session.add_email("INBOX", create_dummy_email(3, "Test3"));
        let params = json!({ "folder": "INBOX", "uids": [1, 3] }); // Select folder explicitly here
        let request = create_request(json!(17), "imap/fetchEmails", params);
        let output = run_adapter_test(&adapter, &request).await;
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
        mock_session.add_folder(Folder { 
            name: "Source".into(), 
            delimiter: Some("/".into()), 
        });
        mock_session.add_folder(Folder { 
            name: "Dest".into(), 
            delimiter: Some("/".into()), 
        });
        mock_session.add_email("Source", create_dummy_email(5, "ToMove"));
        let params = json!({ "source_folder": "Source", "uids": [5], "destination_folder": "Dest" });
        let request = create_request(json!(18), "imap/moveEmail", params);
        let output = run_adapter_test(&adapter, &request).await;
        let response: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(response["id"], 18);
        assert!(response["error"].is_null());
        assert_eq!(response["result"]["destination"], "Dest");
        assert!(mock_session.emails.lock().await.get("Source").unwrap().is_empty());
        assert_eq!(mock_session.emails.lock().await.get("Dest").unwrap().len(), 1);
        assert_eq!(mock_session.emails.lock().await.get("Dest").unwrap()[0].uid, 5);
    }

} 