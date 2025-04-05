use async_trait::async_trait;
use imap_api_rust::{
    imap::client::{ImapSessionTrait, ZeroCopy},
};

pub struct MockImapSession {
    pub should_fail: bool,
}

impl MockImapSession {
    pub fn new() -> Self {
        Self { should_fail: false }
    }
}

#[async_trait]
impl ImapSessionTrait for MockImapSession {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        let names = vec!["INBOX".to_string(), "Sent".to_string(), "Drafts".to_string()];
        Ok(ZeroCopy::from(names))
    }

    async fn create(&self, _name: &str) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }

    async fn delete(&self, _name: &str) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }

    async fn select(&self, _name: &str) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }

    async fn search(&self, _query: &str) -> Result<Vec<u32>, imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(vec![1, 2, 3])
    }

    async fn fetch(&self, _sequence: &str) -> Result<ZeroCopy<Vec<String>>, imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        let fetches = vec!["Email content 1".to_string(), "Email content 2".to_string()];
        Ok(ZeroCopy::from(fetches))
    }

    async fn uid_fetch(&self, _sequence: &str) -> Result<ZeroCopy<Vec<String>>, imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        let fetches = vec!["Email content 1".to_string(), "Email content 2".to_string()];
        Ok(ZeroCopy::from(fetches))
    }

    async fn uid_move(&self, _sequence: &str, _mailbox: &str) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }

    async fn rename(&self, _from: &str, _to: &str) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }

    async fn logout(&self) -> Result<(), imap::error::Error> {
        if self.should_fail {
            return Err(imap::error::Error::Bad("Connection lost".to_string()));
        }
        Ok(())
    }
} 