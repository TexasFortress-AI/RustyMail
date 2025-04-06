use async_trait::async_trait;
use imap_api_rust::imap::client::{ImapClientError, ImapSessionTrait, ZeroCopy};
use mockall::mock;

#[derive(Clone)]
pub struct MockImapSession {
    pub list_result: Result<Vec<String>, ImapClientError>,
    pub create_result: Result<(), ImapClientError>,
    pub delete_result: Result<(), ImapClientError>,
    pub select_result: Result<(), ImapClientError>,
    pub search_result: Result<Vec<u32>, ImapClientError>,
    pub fetch_result: Result<Vec<String>, ImapClientError>,
    pub uid_fetch_result: Result<Vec<String>, ImapClientError>,
    pub uid_move_result: Result<(), ImapClientError>,
    pub rename_result: Result<(), ImapClientError>,
    pub logout_result: Result<(), ImapClientError>,
}

impl MockImapSession {
    pub fn new() -> Self {
        Self {
            list_result: Ok(vec!["INBOX".to_string()]),
            create_result: Ok(()),
            delete_result: Ok(()),
            select_result: Ok(()),
            search_result: Ok(vec![1, 2, 3]),
            fetch_result: Ok(vec!["Email content".to_string()]),
            uid_fetch_result: Ok(vec!["Email content".to_string()]),
            uid_move_result: Ok(()),
            rename_result: Ok(()),
            logout_result: Ok(()),
        }
    }
}

#[async_trait]
impl ImapSessionTrait for MockImapSession {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.list_result.clone().map(ZeroCopy::from)
    }

    async fn create(&self, _name: &str) -> Result<(), ImapClientError> {
        self.create_result.clone()
    }

    async fn delete(&self, _name: &str) -> Result<(), ImapClientError> {
        self.delete_result.clone()
    }

    async fn select(&self, _name: &str) -> Result<(), ImapClientError> {
        self.select_result.clone()
    }

    async fn search(&self, _query: &str) -> Result<Vec<u32>, ImapClientError> {
        self.search_result.clone()
    }

    async fn fetch(&self, _sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.fetch_result.clone().map(ZeroCopy::from)
    }

    async fn uid_fetch(&self, _sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.uid_fetch_result.clone().map(ZeroCopy::from)
    }

    async fn uid_move(&self, _sequence: &str, _mailbox: &str) -> Result<(), ImapClientError> {
        self.uid_move_result.clone()
    }

    async fn rename(&self, _from: &str, _to: &str) -> Result<(), ImapClientError> {
        self.rename_result.clone()
    }

    async fn logout(&self) -> Result<(), ImapClientError> {
        self.logout_result.clone()
    }
}

pub type MockImapSessionWrapper = MockImapSession; 