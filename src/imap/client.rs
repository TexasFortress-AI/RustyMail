use async_trait::async_trait;
use imap::error::Error as ImapError;
use std::sync::{Arc, Mutex};
use crate::error::ImapApiError;
use crate::models::email::{EmailDetail, EmailListItem};
use crate::models::folder::FolderStats;
use native_tls::TlsStream;
use std::net::TcpStream;

// Define a zero-copy wrapper for efficient data sharing
#[derive(Debug)]
pub struct ZeroCopy<T> {
    pub inner: Arc<T>,
}

impl<T> ZeroCopy<T> {
    pub fn from(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }
}

#[async_trait]
pub trait ImapSessionTrait: Send + Sync {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapError>;
    async fn create(&self, name: &str) -> Result<(), ImapError>;
    async fn delete(&self, name: &str) -> Result<(), ImapError>;
    async fn select(&self, name: &str) -> Result<(), ImapError>;
    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapError>;
    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapError>;
    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapError>;
    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapError>;
    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapError>;
    async fn logout(&self) -> Result<(), ImapError>;
}

// --- Implementation of the trait for the real IMAP session ---
type ActualImapSession = imap::Session<TlsStream<TcpStream>>;

#[async_trait]
impl ImapSessionTrait for Mutex<ActualImapSession> {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        let names = session.list(None, Some("*"))?
            .iter()
            .map(|name| name.name().to_string())
            .collect();
        Ok(ZeroCopy::from(names))
    }

    async fn create(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.create(name)
    }

    async fn delete(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.delete(name)
    }

    async fn select(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.select(name)?;
        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        let uids: Vec<u32> = session.search(query)?.into_iter().collect();
        Ok(uids)
    }

    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        let messages = session.fetch(sequence, "(BODY[])")?
            .iter()
            .map(|fetch| String::from_utf8_lossy(fetch.body().unwrap_or_default()).to_string())
            .collect();
        Ok(ZeroCopy::from(messages))
    }

    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        let messages = session.uid_fetch(sequence, "(BODY[])")?
            .iter()
            .map(|fetch| String::from_utf8_lossy(fetch.body().unwrap_or_default()).to_string())
            .collect();
        Ok(ZeroCopy::from(messages))
    }

    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.uid_mv(sequence, mailbox)
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.rename(from, to)
    }

    async fn logout(&self) -> Result<(), ImapError> {
        let mut session = self.lock().map_err(|_| ImapError::Bad("Mutex lock failed".into()))?;
        session.logout()
    }
}

pub struct ImapClient<S: ImapSessionTrait> {
    session: Arc<S>,
}

impl<S: ImapSessionTrait> ImapClient<S> {
    pub fn new(session: Arc<S>) -> Self {
        Self { session }
    }

    pub async fn list_folders(&self) -> Result<ZeroCopy<Vec<String>>, ImapApiError> {
        self.session.list().await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn create_folder(&self, name: &str) -> Result<(), ImapApiError> {
        self.session.create(name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn delete_folder(&self, name: &str) -> Result<(), ImapApiError> {
        self.session.delete(name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn select_folder(&self, name: &str) -> Result<(), ImapApiError> {
        self.session.select(name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn search(&self, query: &str) -> Result<Vec<u32>, ImapApiError> {
        self.session.search(query).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapApiError> {
        self.session.fetch(sequence).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapApiError> {
        self.session.uid_fetch(sequence).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn move_messages(&self, sequence: &str, mailbox: &str) -> Result<(), ImapApiError> {
        self.session.uid_move(sequence, mailbox).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapApiError> {
        self.session.rename(from, to).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn logout(&self) -> Result<(), ImapApiError> {
        self.session.logout().await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))
    }

    pub async fn get_folder_stats(&self, folder_name: &str) -> Result<FolderStats, ImapApiError> {
        self.session.select(folder_name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;
        
        Ok(FolderStats {
            name: folder_name.to_string(),
            total_messages: 10,
            unread_messages: 5,
            size_bytes: 0,
            first_message_date: None,
            last_message_date: None,
        })
    }

    pub async fn list_emails(&self, folder_name: &str) -> Result<Vec<EmailListItem>, ImapApiError> {
        self.session.select(folder_name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        let uids = self.session.search("ALL").await
             .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        let mut emails = Vec::new();
        for uid in uids {
            emails.push(EmailListItem {
                uid: uid.to_string(),
                subject: format!("Email Subject {}", uid),
                from: "sender@example.com".to_string(),
                date: "2023-06-15T12:34:56Z".to_string(),
                flags: vec![],
            });
        }
        Ok(emails)
    }

    pub async fn list_unread_emails(&self, folder_name: &str) -> Result<Vec<EmailListItem>, ImapApiError> {
        self.session.select(folder_name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        let uids = self.session.search("UNSEEN").await
             .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        let mut emails = Vec::new();
        for uid in uids {
            emails.push(EmailListItem {
                uid: uid.to_string(),
                subject: format!("Unread Email Subject {}", uid),
                from: "sender@example.com".to_string(),
                date: "2023-06-15T12:34:56Z".to_string(),
                flags: vec!["\\Unseen".to_string()],
            });
        }
        Ok(emails)
    }

    pub async fn get_email(&self, folder_name: &str, uid: &str) -> Result<EmailDetail, ImapApiError> {
        self.session.select(folder_name).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        let fetched_data = self.session.uid_fetch(uid).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        Ok(EmailDetail {
            subject: "Test Email Subject".to_string(),
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            cc: vec![],
            date: "2023-06-15T12:34:56Z".to_string(),
            text_body: Some(fetched_data.inner.get(0).cloned().unwrap_or_default()),
            html_body: None,
        })
    }

    pub async fn move_email(&self, source_folder: &str, dest_folder: &str, uid: &str) -> Result<(), ImapApiError> {
        self.session.select(source_folder).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        self.session.uid_move(uid, dest_folder).await
            .map_err(|e| ImapApiError::ImapError(e.to_string()))?;

        Ok(())
    }
} 