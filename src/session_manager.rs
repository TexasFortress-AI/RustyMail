use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::config::Settings;
use crate::imap::{ImapClient, ImapError, AsyncImapSessionWrapper};

/// Result type for session operations
pub type SessionResult<T> = Result<T, SessionError>;

/// Errors that can occur during session management
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found for API key")]
    NotFound,
    #[error("Failed to create session: {0}")]
    Creation(#[from] ImapError),
    #[error("Failed to access session: {0}")]
    Access(String),
}

/// Type alias for the standard ImapClient we'll be managing
pub type ManagedClient = ImapClient<AsyncImapSessionWrapper>;

/// Trait defining session management operations
#[async_trait]
pub trait SessionManagerTrait: Send + Sync {
    /// Get an existing session for the given API key
    async fn get_session(&self, api_key: &str) -> SessionResult<Arc<ManagedClient>>;
    
    /// Create a new session with the given credentials
    async fn create_session(
        &self, 
        api_key: &str, 
        username: &str, 
        password: &str, 
        server: &str, 
        port: u16
    ) -> SessionResult<Arc<ManagedClient>>;
    
    /// Remove a session for the given API key
    async fn remove_session(&self, api_key: &str) -> SessionResult<()>;
}

/// Session manager that maintains IMAP client sessions by API key
#[derive(Debug)]
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<ManagedClient>>>>,
    settings: Arc<Settings>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            settings,
        }
    }
}

#[async_trait]
impl SessionManagerTrait for SessionManager {
    async fn get_session(&self, api_key: &str) -> SessionResult<Arc<ManagedClient>> {
        let sessions = self.sessions.lock().await;
        
        match sessions.get(api_key) {
            Some(client) => {
                debug!("Retrieved existing session for API key");
                Ok(Arc::clone(client))
            }
            None => {
                warn!("No session found for API key");
                Err(SessionError::NotFound)
            }
        }
    }
    
    async fn create_session(
        &self, 
        api_key: &str, 
        username: &str, 
        password: &str, 
        server: &str, 
        port: u16
    ) -> SessionResult<Arc<ManagedClient>> {
        info!("Creating new IMAP session for API key");
        
        let client = ImapClient::<AsyncImapSessionWrapper>::connect(server, port, username, password).await?;
        let client = Arc::new(client);
        
        let mut sessions = self.sessions.lock().await;
        sessions.insert(api_key.to_string(), Arc::clone(&client));
        
        Ok(client)
    }
    
    async fn remove_session(&self, api_key: &str) -> SessionResult<()> {
        let mut sessions = self.sessions.lock().await;
        
        if sessions.remove(api_key).is_some() {
            info!("Removed session for API key");
            Ok(())
        } else {
            warn!("Attempted to remove non-existent session for API key");
            Err(SessionError::NotFound)
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(Arc::new(Settings::default()))
    }
}

// Re-export MockSessionManager for tests
#[cfg(test)]
pub use mock::MockSessionManager;

// Mock implementation for testing
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    /// Trait for mocking session manager behavior in tests
    #[async_trait]
    pub trait MockSessionManagerTrait: Send + Sync {
        /// Get the number of times get_session was called
        fn get_session_call_count(&self) -> usize;
        
        /// Get the number of times create_session was called
        fn create_session_call_count(&self) -> usize;
        
        /// Get the number of times remove_session was called
        fn remove_session_call_count(&self) -> usize;
        
        /// Set what get_session should return
        fn mock_get_session(&self, api_key: &str, result: SessionResult<Arc<ManagedClient>>);
        
        /// Set what create_session should return
        fn mock_create_session(&self, result: SessionResult<Arc<ManagedClient>>);
        
        /// Set what remove_session should return
        fn mock_remove_session(&self, result: SessionResult<()>);
    }
    
    /// Mock session manager for testing
    pub struct MockSessionManager {
        get_session_count: AtomicUsize,
        create_session_count: AtomicUsize,
        remove_session_count: AtomicUsize,
        
        // Mock responses indexed by API key
        get_session_responses: Arc<Mutex<HashMap<String, SessionResult<Arc<ManagedClient>>>>>,
        create_session_response: Arc<Mutex<Option<SessionResult<Arc<ManagedClient>>>>>,
        remove_session_response: Arc<Mutex<Option<SessionResult<()>>>>,
    }
    
    impl MockSessionManager {
        pub fn new() -> Self {
            Self {
                get_session_count: AtomicUsize::new(0),
                create_session_count: AtomicUsize::new(0),
                remove_session_count: AtomicUsize::new(0),
                get_session_responses: Arc::new(Mutex::new(HashMap::new())),
                create_session_response: Arc::new(Mutex::new(None)),
                remove_session_response: Arc::new(Mutex::new(None)),
            }
        }
    }
    
    impl Default for MockSessionManager {
        fn default() -> Self {
            Self::new()
        }
    }
    
    #[async_trait]
    impl MockSessionManagerTrait for MockSessionManager {
        fn get_session_call_count(&self) -> usize {
            self.get_session_count.load(Ordering::SeqCst)
        }
        
        fn create_session_call_count(&self) -> usize {
            self.create_session_count.load(Ordering::SeqCst)
        }
        
        fn remove_session_call_count(&self) -> usize {
            self.remove_session_count.load(Ordering::SeqCst)
        }
        
        fn mock_get_session(&self, api_key: &str, result: SessionResult<Arc<ManagedClient>>) {
            let mut responses = self.get_session_responses.try_lock()
                .expect("Failed to lock get_session_responses");
            responses.insert(api_key.to_string(), result);
        }
        
        fn mock_create_session(&self, result: SessionResult<Arc<ManagedClient>>) {
            let mut response = self.create_session_response.try_lock()
                .expect("Failed to lock create_session_response");
            *response = Some(result);
        }
        
        fn mock_remove_session(&self, result: SessionResult<()>) {
            let mut response = self.remove_session_response.try_lock()
                .expect("Failed to lock remove_session_response");
            *response = Some(result);
        }
    }
    
    #[async_trait]
    impl SessionManagerTrait for MockSessionManager {
        async fn get_session(&self, api_key: &str) -> SessionResult<Arc<ManagedClient>> {
            self.get_session_count.fetch_add(1, Ordering::SeqCst);
            
            let responses = self.get_session_responses.lock().await;
            if let Some(result) = responses.get(api_key) {
                match result {
                    Ok(client) => Ok(client.clone()),
                    Err(err) => Err(SessionError::Access(err.to_string())),
                }
            } else {
                Err(SessionError::NotFound)
            }
        }
        
        async fn create_session(
            &self,
            api_key: &str,
            username: &str,
            password: &str,
            server: &str,
            port: u16
        ) -> SessionResult<Arc<ManagedClient>> {
            self.create_session_count.fetch_add(1, Ordering::SeqCst);
            
            let response = self.create_session_response.lock().await;
            match &*response {
                Some(Ok(client)) => Ok(client.clone()),
                Some(Err(_err)) => Err(SessionError::Creation(ImapError::Connection("Mock error".to_string()))),
                None => Err(SessionError::NotFound),
            }
        }
        
        async fn remove_session(&self, api_key: &str) -> SessionResult<()> {
            self.remove_session_count.fetch_add(1, Ordering::SeqCst);
            
            let response = self.remove_session_response.lock().await;
            match &*response {
                Some(result) => result.clone(),
                None => Err(SessionError::NotFound),
            }
        }
    }
} 