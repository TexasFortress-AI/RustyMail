use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tokio::time::sleep;

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
#[derive(Debug)]
struct PooledConnection {
    client: Arc<ImapClient<AsyncImapSessionWrapper>>,
    created_at: Instant,
    last_used: Instant,
    is_healthy: bool,
}

impl PooledConnection {
    fn new(client: Arc<ImapClient<AsyncImapSessionWrapper>>) -> Self {
        let now = Instant::now();
        Self {
            client,
            created_at: now,
            last_used: now,
            is_healthy: true,
        }
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    fn is_expired(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }
}

/// Connection factory trait for creating new connections
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
    async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError>;
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
}

/// Connection pool implementation using Arc<TokioMutex<>>
pub struct ConnectionPool {
    /// Available connections
    connections: Arc<TokioMutex<VecDeque<PooledConnection>>>,
    /// Factory for creating new connections
    factory: Arc<dyn ConnectionFactory>,
    /// Pool configuration
    config: PoolConfig,
    /// Semaphore to limit total connections
    semaphore: Arc<Semaphore>,
    /// Flag to indicate if pool is shutting down
    is_shutting_down: Arc<TokioMutex<bool>>,
    /// Active connection count
    active_count: Arc<TokioMutex<usize>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(factory: Arc<dyn ConnectionFactory>, config: PoolConfig) -> Arc<Self> {
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let pool = Arc::new(Self {
            connections: Arc::new(TokioMutex::new(VecDeque::new())),
            factory,
            config: config.clone(),
            semaphore,
            is_shutting_down: Arc::new(TokioMutex::new(false)),
            active_count: Arc::new(TokioMutex::new(0)),
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

        pool
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, PoolError> {
        // Check if shutting down
        if *self.is_shutting_down.lock().await {
            return Err(PoolError::ShuttingDown);
        }

        // Try to get an existing connection
        let mut connections = self.connections.lock().await;
        while let Some(mut conn) = connections.pop_front() {
            if !conn.is_expired(self.config.idle_timeout) && conn.is_healthy {
                conn.touch();
                let client = conn.client.clone();

                // Track active connection
                *self.active_count.lock().await += 1;

                // Return connection to pool when done
                let connections_clone = Arc::clone(&self.connections);
                let active_count_clone = Arc::clone(&self.active_count);
                let conn_wrapper = conn;

                tokio::spawn(async move {
                    // In a real implementation, we'd use a guard pattern here
                    // For now, we'll just return the connection immediately
                    connections_clone.lock().await.push_back(conn_wrapper);
                    *active_count_clone.lock().await -= 1;
                });

                debug!("Acquired connection from pool");
                return Ok(client);
            }
        }
        drop(connections);

        // No available connection, try to create a new one
        let permit = self.semaphore.try_acquire();
        if permit.is_err() {
            warn!("Connection pool exhausted");
            return Err(PoolError::PoolExhausted);
        }

        // Create new connection
        match self.factory.create().await {
            Ok(client) => {
                info!("Created new connection for pool");
                *self.active_count.lock().await += 1;

                // Store for reuse
                let conn = PooledConnection::new(client.clone());
                self.connections.lock().await.push_back(conn);

                Ok(client)
            }
            Err(e) => {
                error!("Failed to create connection: {}", e);
                Err(PoolError::ConnectionFailed(e.to_string()))
            }
        }
    }

    /// Maintain minimum pool size
    async fn maintain_pool(self: Arc<Self>) {
        loop {
            if *self.is_shutting_down.lock().await {
                break;
            }

            let pool_size = self.connections.lock().await.len();
            let active = *self.active_count.lock().await;
            let total = pool_size + active;

            if total < self.config.min_connections {
                let needed = self.config.min_connections - total;
                debug!("Pool below minimum, creating {} connections", needed);

                for _ in 0..needed {
                    if let Ok(client) = self.factory.create().await {
                        let conn = PooledConnection::new(client);
                        self.connections.lock().await.push_back(conn);
                    }
                }
            }

            // Clean up expired connections
            let mut connections = self.connections.lock().await;
            connections.retain(|conn| {
                !conn.is_expired(self.config.idle_timeout)
            });

            drop(connections);
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

            let mut connections = self.connections.lock().await;
            for conn in connections.iter_mut() {
                // In a real implementation, we'd perform an actual health check
                // For now, we'll just mark connections older than 5 minutes as potentially unhealthy
                if conn.created_at.elapsed() > Duration::from_secs(300) {
                    conn.is_healthy = false;
                    debug!("Marked connection as unhealthy");
                }
            }

            // Remove unhealthy connections
            connections.retain(|conn| conn.is_healthy);
        }
    }

    /// Shutdown the pool gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down connection pool");
        *self.is_shutting_down.lock().await = true;

        // Clear all connections
        self.connections.lock().await.clear();
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let available = self.connections.lock().await.len();
        let active = *self.active_count.lock().await;

        PoolStats {
            available_connections: available,
            active_connections: active,
            total_connections: available + active,
            max_connections: self.config.max_connections,
        }
    }
}

/// Statistics about the pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available_connections: usize,
    pub active_connections: usize,
    pub total_connections: usize,
    pub max_connections: usize,
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