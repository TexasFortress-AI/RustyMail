use imap_api_rust::imap::client::{ImapClient, ImapSessionTrait, ZeroCopy};
use imap_api_rust::error::ImapApiError;
use mockall::mock;
use mockall::predicate::*;
use async_trait::async_trait;
use std::sync::Arc;

mock! {
    #[async_trait]
    pub ImapSession {}

    #[async_trait]
    impl ImapSessionTrait for ImapSession {
        async fn list(&self) -> Result<ZeroCopy<Vec<String>>, imap::error::Error>;
        async fn create(&self, name: &str) -> Result<(), imap::error::Error>;
        async fn delete(&self, name: &str) -> Result<(), imap::error::Error>;
        async fn select(&self, name: &str) -> Result<(), imap::error::Error>;
        async fn search(&self, query: &str) -> Result<Vec<u32>, imap::error::Error>;
        async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, imap::error::Error>;
        async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, imap::error::Error>;
        async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), imap::error::Error>;
        async fn rename(&self, from: &str, to: &str) -> Result<(), imap::error::Error>;
        async fn logout(&self) -> Result<(), imap::error::Error>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_list_folders() {
        let mut mock = MockImapSession::new();
        mock.expect_list()
            .returning(|| {
                Ok(ZeroCopy::from(vec!["INBOX".to_string(), "Sent".to_string()]))
            });

        let client = ImapClient::new(Arc::new(mock));
        let result = client.list_folders().await;
        assert!(result.is_ok());
        let folders = result.unwrap();
        assert_eq!(folders.inner.len(), 2);
        assert_eq!(folders.inner[0], "INBOX");
    }

    #[tokio::test]
    async fn test_create_folder() {
        let mut mock = MockImapSession::new();
        mock.expect_create()
            .with(eq("NewFolder"))
            .returning(|_| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.create_folder("NewFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_folder() {
        let mut mock = MockImapSession::new();
        mock.expect_delete()
            .with(eq("OldFolder"))
            .returning(|_| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.delete_folder("OldFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_select_folder() {
        let mut mock = MockImapSession::new();
        mock.expect_select()
            .with(eq("INBOX"))
            .returning(|_| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.select_folder("INBOX").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_search() {
        let mut mock = MockImapSession::new();
        mock.expect_search()
            .with(eq("ALL"))
            .returning(|_| Ok(vec![1, 2, 3]));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.search("ALL").await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
    }

    #[tokio::test]
    async fn test_fetch() {
        let mut mock = MockImapSession::new();
        mock.expect_fetch()
            .with(eq("1"))
            .returning(|_| {
                Ok(ZeroCopy::from(vec!["Email Body 1".to_string()]))
            });

        let client = ImapClient::new(Arc::new(mock));
        let result = client.fetch("1").await;
        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched.inner.len(), 1);
        assert_eq!(fetched.inner[0], "Email Body 1");
    }

    #[tokio::test]
    async fn test_move_messages() {
        let mut mock = MockImapSession::new();
        mock.expect_uid_move()
            .with(eq("1"), eq("Trash"))
            .returning(|_, _| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.move_messages("1", "Trash").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connection_error_list() {
        let mut mock = MockImapSession::new();
        mock.expect_list()
            .returning(|| Err(imap::error::Error::Bad("Connection error".to_string())));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.list_folders().await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ImapApiError::ImapError(msg) => assert!(msg.contains("Connection error")),
            _ => panic!("Expected ImapError"),
        }
    }

    #[tokio::test]
    async fn test_search_unread() {
        let mut mock = MockImapSession::new();
        mock.expect_search()
            .with(eq("UNSEEN"))
            .returning(|_| Ok(vec![1, 2]));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.search("UNSEEN").await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[tokio::test]
    async fn test_get_single_email_fetch() {
        let mut mock = MockImapSession::new();
        mock.expect_select()
            .with(eq("INBOX"))
            .returning(|_| Ok(()));
        mock.expect_uid_fetch()
            .with(eq("1"))
            .returning(|_| Ok(ZeroCopy::from(vec!["Email Body 1".to_string()])));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.get_email("INBOX", "1").await;
        assert!(result.is_ok());
        let detail = result.unwrap();
        assert_eq!(detail.subject, "Test Email Subject");
    }

    #[tokio::test]
    async fn test_invalid_folder_select() {
        let mut mock = MockImapSession::new();
        mock.expect_select()
            .with(eq("InvalidFolder"))
            .returning(|_| Err(imap::error::Error::No("Folder does not exist".to_string())));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.select_folder("InvalidFolder").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ImapApiError::ImapError(msg) => assert!(msg.contains("Folder does not exist")),
            _ => panic!("Expected ImapError"),
        }
    }

    #[tokio::test]
    async fn test_invalid_uid_fetch() {
        let mut mock = MockImapSession::new();
        mock.expect_select()
            .with(eq("INBOX"))
            .returning(|_| Ok(()));
        mock.expect_uid_fetch()
            .with(eq("99999"))
            .returning(|_| Err(imap::error::Error::No("Message not found".to_string())));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.get_email("INBOX", "99999").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ImapApiError::ImapError(msg) => assert!(msg.contains("Message not found")),
            _ => panic!("Expected ImapError"),
        }
    }

    #[tokio::test]
    async fn test_invalid_move() {
        let mut mock = MockImapSession::new();
        mock.expect_uid_move()
            .with(eq("99999"), eq("Trash"))
            .returning(|_, _| Err(imap::error::Error::No("Message not found".to_string())));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.move_messages("99999", "Trash").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ImapApiError::ImapError(msg) => assert!(msg.contains("Message not found")),
            _ => panic!("Expected ImapError"),
        }
    }

    #[tokio::test]
    async fn test_delete_non_empty_folder() {
        let mut mock = MockImapSession::new();
        mock.expect_delete()
            .with(eq("NonEmptyFolder"))
            .returning(|_| Err(imap::error::Error::No("Folder is not empty".to_string())));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.delete_folder("NonEmptyFolder").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ImapApiError::ImapError(msg) => assert!(msg.contains("Folder is not empty")),
            _ => panic!("Expected ImapError"),
        }
    }

    #[tokio::test]
    async fn test_rename_folder() {
        let mut mock = MockImapSession::new();
        mock.expect_rename()
            .with(eq("OldFolder"), eq("NewFolder"))
            .returning(|_, _| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.rename_folder("OldFolder", "NewFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_folder_stats() {
        let mut mock = MockImapSession::new();
        mock.expect_select()
            .with(eq("INBOX"))
            .returning(|_| Ok(()));

        let client = ImapClient::new(Arc::new(mock));
        let result = client.get_folder_stats("INBOX").await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.name, "INBOX");
        assert_eq!(stats.total_messages, 10);
    }
}