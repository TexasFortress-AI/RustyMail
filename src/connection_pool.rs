use std::collections::{VecDeque, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, Semaphore, RwLock};
use tokio::time::sleep;
use uuid::Uuid;

use crate::imap::{ImapClient, ImapError, AsyncImapSessionWrapper};

/// Errors that can occur during pool operations
#[derive(Debug, Error, Clone)]
pub enum PoolError {
    #[error("Connection pool exhausted")]
    PoolExhausted,
    #[error("Failed to create connection: {0}")]
    ConnectionFailed(String),
    #[error("Connection unhealthy")]
    Unhealthy,
    #[error("Pool is shutting down")]
    ShuttingDown,
}

/// Configuration for the connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Time before an idle connection is closed
    pub idle_timeout: Duration,
    /// Time between health checks
    pub health_check_interval: Duration,
    /// Maximum wait time for acquiring a connection
    pub acquire_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 100,
            idle_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
            acquire_timeout: Duration::from_secs(10),
        }
    }
}

/// A pooled connection with metadata
#[derive(Debug, Clone)]
struct PooledConnection {
    id: Uuid,
    client: Arc<ImapClient<AsyncImapSessionWrapper>>,
    created_at: Instant,
    last_used: Instant,
    is_healthy: bool,
    in_use: bool,
}

impl PooledConnection {
    fn new(client: Arc<ImapClient<AsyncImapSessionWrapper>>) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::new_v4(),
            client,
            created_at: now,
            last_used: now,
            is_healthy: true,
            in_use: false,
        }
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    fn is_expired(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }

    fn mark_in_use(&mut self) {
        self.in_use = true;
        self.touch();
    }

    fn mark_available(&mut self) {
        self.in_use = false;
        self.touch();
    }
}

/// Session handle that ensures proper lifecycle management
pub struct SessionHandle {
    connection_id: Uuid,
    client: Arc<ImapClient<AsyncImapSessionWrapper>>,
    pool: Arc<ConnectionPool>,
}

impl SessionHandle {
    fn new(connection_id: Uuid, client: Arc<ImapClient<AsyncImapSessionWrapper>>, pool: Arc<ConnectionPool>) -> Self {
        Self {
            connection_id,
            client,
            pool,
        }
    }

    /// Get the underlying IMAP client
    pub fn client(&self) -> &Arc<ImapClient<AsyncImapSessionWrapper>> {
        &self.client
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        // Return connection to pool when handle is dropped
        let pool = Arc::clone(&self.pool);
        let connection_id = self.connection_id;

        tokio::spawn(async move {
            pool.release_connection(connection_id).await;
        });
    }
}

/// Connection factory trait for creating new connections
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
    async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError>;

    /// Validate that a connection is still healthy
    async fn validate(&self, client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool;
}

/// Default factory implementation using connection parameters
pub struct ImapConnectionFactory {
    server: String,
    port: u16,
    username: String,
    password: String,
}

impl ImapConnectionFactory {
    pub fn new(server: String, port: u16, username: String, password: String) -> Self {
        Self {
            server,
            port,
            username,
            password,
        }
    }
}

#[async_trait]
impl ConnectionFactory for ImapConnectionFactory {
    async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError> {
        let client = ImapClient::<AsyncImapSessionWrapper>::connect(
            &self.server,
            self.port,
            &self.username,
            &self.password,
        ).await?;
        Ok(Arc::new(client))
    }

    async fn validate(&self, _client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool {
        // TODO: Implement proper connection validation
        // For now, we'll rely on operations failing when connections are bad
        // and the reconnection logic will handle it
        true
    }
}

/// Connection pool implementation using Arc<TokioMutex<>>
pub struct ConnectionPool {
    /// All connections (both available and in-use)
    connections: Arc<RwLock<HashMap<Uuid, PooledConnection>>>,
    /// Queue of available connection IDs
    available: Arc<TokioMutex<VecDeque<Uuid>>>,
    /// Factory for creating new connections
    factory: Arc<dyn ConnectionFactory>,
    /// Pool configuration
    config: PoolConfig,
    /// Semaphore to limit total connections
    semaphore: Arc<Semaphore>,
    /// Flag to indicate if pool is shutting down
    is_shutting_down: Arc<TokioMutex<bool>>,
    /// Statistics
    total_created: Arc<AtomicUsize>,
    total_acquired: Arc<AtomicUsize>,
    total_released: Arc<AtomicUsize>,
    current_active: Arc<AtomicUsize>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(factory: Arc<dyn ConnectionFactory>, config: PoolConfig) -> Arc<Self> {
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let pool = Arc::new(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            available: Arc::new(TokioMutex::new(VecDeque::new())),
            factory,
            config: config.clone(),
            semaphore,
            is_shutting_down: Arc::new(TokioMutex::new(false)),
            total_created: Arc::new(AtomicUsize::new(0)),
            total_acquired: Arc::new(AtomicUsize::new(0)),
            total_released: Arc::new(AtomicUsize::new(0)),
            current_active: Arc::new(AtomicUsize::new(0)),
        });

        // Start background tasks
        let pool_clone = Arc::clone(&pool);
        tokio::spawn(async move {
            pool_clone.maintain_pool().await;
        });

        let pool_clone = Arc::clone(&pool);
        tokio::spawn(async move {
            pool_clone.health_check_loop().await;
        });

        // Pre-warm the pool with minimum connections
        let pool_clone = Arc::clone(&pool);
        tokio::spawn(async move {
            for _ in 0..pool_clone.config.min_connections {
                if let Err(e) = pool_clone.create_connection().await {
                    warn!("Failed to pre-warm connection: {}", e);
                }
            }
        });

        pool
    }

    /// Create a new connection and add to pool
    async fn create_connection(&self) -> Result<Uuid, PoolError> {
        // Check semaphore
        let _permit = self.semaphore.acquire().await
            .map_err(|_| PoolError::PoolExhausted)?;

        // Create new connection
        let client = self.factory.create().await
            .map_err(|e| PoolError::ConnectionFailed(e.to_string()))?;

        let conn = PooledConnection::new(client);
        let conn_id = conn.id;

        // Add to connections map
        self.connections.write().await.insert(conn_id, conn);

        // Add to available queue
        self.available.lock().await.push_back(conn_id);

        self.total_created.fetch_add(1, Ordering::SeqCst);
        info!("Created new connection {}", conn_id);

        Ok(conn_id)
    }

    /// Acquire a session handle from the pool
    pub async fn acquire(self: Arc<Self>) -> Result<SessionHandle, PoolError> {
        // Check if shutting down
        if *self.is_shutting_down.lock().await {
            return Err(PoolError::ShuttingDown);
        }

        // Try to get an available connection
        loop {
            let conn_id = {
                let mut available = self.available.lock().await;
                available.pop_front()
            };

            if let Some(conn_id) = conn_id {
                let mut connections = self.connections.write().await;
                if let Some(conn) = connections.get_mut(&conn_id) {
                    // Check if connection is still valid
                    if !conn.is_expired(self.config.idle_timeout) && conn.is_healthy {
                        conn.mark_in_use();
                        let client = conn.client.clone();

                        self.total_acquired.fetch_add(1, Ordering::SeqCst);
                        self.current_active.fetch_add(1, Ordering::SeqCst);

                        debug!("Acquired connection {} from pool", conn_id);
                        return Ok(SessionHandle::new(conn_id, client, Arc::clone(&self)));
                    } else {
                        // Remove expired/unhealthy connection
                        connections.remove(&conn_id);
                        debug!("Removed expired/unhealthy connection {}", conn_id);
                    }
                }
            } else {
                // No available connections, try to create a new one
                let total = self.connections.read().await.len();
                if total < self.config.max_connections {
                    match self.create_connection().await {
                        Ok(new_conn_id) => {
                            // Try to acquire the newly created connection
                            continue;
                        }
                        Err(e) => {
                            error!("Failed to create new connection: {}", e);
                            return Err(e);
                        }
                    }
                } else {
                    warn!("Connection pool exhausted");
                    return Err(PoolError::PoolExhausted);
                }
            }
        }
    }

    /// Release a connection back to the pool
    async fn release_connection(&self, connection_id: Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&connection_id) {
            conn.mark_available();
            drop(connections); // Release write lock

            self.available.lock().await.push_back(connection_id);
            self.total_released.fetch_add(1, Ordering::SeqCst);
            self.current_active.fetch_sub(1, Ordering::SeqCst);

            debug!("Released connection {} back to pool", connection_id);
        } else {
            warn!("Attempted to release unknown connection {}", connection_id);
        }
    }

    /// Maintain minimum pool size
    async fn maintain_pool(self: Arc<Self>) {
        loop {
            if *self.is_shutting_down.lock().await {
                break;
            }

            let total_connections = self.connections.read().await.len();
            let active = self.current_active.load(Ordering::SeqCst);

            if total_connections < self.config.min_connections {
                let needed = self.config.min_connections - total_connections;
                debug!("Pool below minimum, creating {} connections", needed);

                for _ in 0..needed {
                    if let Err(e) = self.create_connection().await {
                        warn!("Failed to maintain minimum pool size: {}", e);
                    }
                }
            }

            // Clean up expired connections
            let mut expired_ids = Vec::new();
            {
                let connections = self.connections.read().await;
                for (id, conn) in connections.iter() {
                    if conn.is_expired(self.config.idle_timeout) && !conn.in_use {
                        expired_ids.push(*id);
                    }
                }
            }

            if !expired_ids.is_empty() {
                let mut connections = self.connections.write().await;
                let mut available = self.available.lock().await;

                for id in expired_ids {
                    connections.remove(&id);
                    available.retain(|&conn_id| conn_id != id);
                    debug!("Removed expired connection {}", id);
                }
            }

            sleep(Duration::from_secs(10)).await;
        }
    }

    /// Periodic health checking of connections
    async fn health_check_loop(self: Arc<Self>) {
        loop {
            if *self.is_shutting_down.lock().await {
                break;
            }

            sleep(self.config.health_check_interval).await;

            let mut to_check = Vec::new();
            {
                let connections = self.connections.read().await;
                for (id, conn) in connections.iter() {
                    // Only check connections that are available and haven't been checked recently
                    if !conn.in_use && conn.last_used.elapsed() > Duration::from_secs(30) {
                        to_check.push((*id, conn.client.clone()));
                    }
                }
            }

            let mut unhealthy_ids = Vec::new();
            let mut to_reconnect = Vec::new();

            // Perform actual health checks
            for (id, client) in to_check {
                if !self.factory.validate(&client).await {
                    unhealthy_ids.push(id);
                    to_reconnect.push(id);
                    warn!("Connection {} failed health check", id);
                } else {
                    debug!("Connection {} passed health check", id);
                }
            }

            // Mark connections as unhealthy
            if !unhealthy_ids.is_empty() {
                let mut connections = self.connections.write().await;
                for id in &unhealthy_ids {
                    if let Some(conn) = connections.get_mut(id) {
                        conn.is_healthy = false;
                    }
                }
            }

            // Attempt to reconnect unhealthy connections
            for id in to_reconnect {
                tokio::spawn({
                    let pool = Arc::clone(&self);
                    async move {
                        pool.reconnect(id).await;
                    }
                });
            }
        }
    }

    /// Attempt to reconnect a failed connection
    async fn reconnect(&self, connection_id: Uuid) {
        info!("Attempting to reconnect connection {}", connection_id);

        // Remove the old connection
        {
            let mut connections = self.connections.write().await;
            if let Some(old_conn) = connections.remove(&connection_id) {
                if old_conn.in_use {
                    warn!("Cannot reconnect in-use connection {}", connection_id);
                    connections.insert(connection_id, old_conn);
                    return;
                }
            }
        }

        // Remove from available queue
        {
            let mut available = self.available.lock().await;
            available.retain(|&id| id != connection_id);
        }

        // Try to create a new connection
        let max_retries = 3;
        for attempt in 1..=max_retries {
            match self.factory.create().await {
                Ok(new_client) => {
                    let mut new_conn = PooledConnection::new(new_client);
                    new_conn.id = connection_id; // Reuse the same ID for tracking

                    // Add the new connection
                    self.connections.write().await.insert(connection_id, new_conn);
                    self.available.lock().await.push_back(connection_id);

                    info!("Successfully reconnected connection {} on attempt {}", connection_id, attempt);
                    return;
                }
                Err(e) => {
                    warn!("Reconnection attempt {} failed for connection {}: {}", attempt, connection_id, e);
                    if attempt < max_retries {
                        sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    }
                }
            }
        }

        error!("Failed to reconnect connection {} after {} attempts", connection_id, max_retries);
    }

    /// Shutdown the pool gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down connection pool");
        *self.is_shutting_down.lock().await = true;

        // Clear all connections
        self.connections.write().await.clear();
        self.available.lock().await.clear();

        info!("Connection pool shutdown complete");
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let connections = self.connections.read().await;
        let total = connections.len();
        let active = self.current_active.load(Ordering::SeqCst);
        let available = self.available.lock().await.len();

        PoolStats {
            available_connections: available,
            active_connections: active,
            total_connections: total,
            max_connections: self.config.max_connections,
            total_created: self.total_created.load(Ordering::SeqCst),
            total_acquired: self.total_acquired.load(Ordering::SeqCst),
            total_released: self.total_released.load(Ordering::SeqCst),
        }
    }

    /// Get detailed session information
    pub async fn get_sessions(&self) -> Vec<SessionInfo> {
        let connections = self.connections.read().await;
        connections
            .values()
            .map(|conn| SessionInfo {
                id: conn.id,
                created_at: conn.created_at,
                last_used: conn.last_used,
                is_healthy: conn.is_healthy,
                in_use: conn.in_use,
            })
            .collect()
    }
}

/// Statistics about the pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available_connections: usize,
    pub active_connections: usize,
    pub total_connections: usize,
    pub max_connections: usize,
    pub total_created: usize,
    pub total_acquired: usize,
    pub total_released: usize,
}

/// Information about a session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub created_at: Instant,
    pub last_used: Instant,
    pub is_healthy: bool,
    pub in_use: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConnectionFactory;

    #[async_trait]
    impl ConnectionFactory for MockConnectionFactory {
        async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError> {
            // In real tests, we'd use a mock client
            Err(ImapError::Connection("Mock factory".to_string()))
        }

        async fn validate(&self, _client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool {
            // Mock validation always returns true
            true
        }
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let factory = Arc::new(MockConnectionFactory);
        let config = PoolConfig::default();
        let pool = ConnectionPool::new(factory, config);

        let stats = pool.stats().await;
        assert_eq!(stats.max_connections, 100);
    }
}