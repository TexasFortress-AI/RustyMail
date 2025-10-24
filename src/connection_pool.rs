// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use crossbeam::queue::ArrayQueue;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, Semaphore};
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
    /// Maximum duration a session can be active
    pub max_session_duration: Duration,
    /// Maximum number of concurrent connection creations allowed
    pub max_concurrent_creations: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 20,  // Increased for high-concurrency scenarios
            max_connections: 100,
            idle_timeout: Duration::from_secs(300),      // 5 minutes
            health_check_interval: Duration::from_secs(30),
            acquire_timeout: Duration::from_secs(5),     // Reduced for faster failures under load
            max_session_duration: Duration::from_secs(3600), // 1 hour
            max_concurrent_creations: 10, // Prevent connection creation storms
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
    /// All connections (both available and in-use) - lock-free concurrent map
    connections: Arc<DashMap<Uuid, PooledConnection>>,
    /// Queue of available connection IDs - lock-free queue
    available: Arc<ArrayQueue<Uuid>>,
    /// Factory for creating new connections
    factory: Arc<dyn ConnectionFactory>,
    /// Pool configuration
    config: PoolConfig,
    /// Semaphore to limit total connections
    semaphore: Arc<Semaphore>,
    /// Semaphore to limit concurrent connection creation
    creation_semaphore: Arc<Semaphore>,
    /// Flag to indicate if pool is shutting down
    is_shutting_down: Arc<TokioMutex<bool>>,
    /// Statistics
    total_created: Arc<AtomicUsize>,
    total_acquired: Arc<AtomicUsize>,
    total_released: Arc<AtomicUsize>,
    current_active: Arc<AtomicUsize>,
    /// High-concurrency metrics
    acquire_timeouts: Arc<AtomicUsize>,
    creation_failures: Arc<AtomicUsize>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(factory: Arc<dyn ConnectionFactory>, config: PoolConfig) -> Arc<Self> {
        let semaphore = Arc::new(Semaphore::new(config.max_connections));
        let creation_semaphore = Arc::new(Semaphore::new(config.max_concurrent_creations));
        // Use max_connections as queue capacity - should be sufficient
        let available_queue = Arc::new(ArrayQueue::new(config.max_connections));

        let pool = Arc::new(Self {
            connections: Arc::new(DashMap::new()),
            available: available_queue,
            factory,
            config: config.clone(),
            semaphore,
            creation_semaphore,
            is_shutting_down: Arc::new(TokioMutex::new(false)),
            total_created: Arc::new(AtomicUsize::new(0)),
            total_acquired: Arc::new(AtomicUsize::new(0)),
            total_released: Arc::new(AtomicUsize::new(0)),
            current_active: Arc::new(AtomicUsize::new(0)),
            acquire_timeouts: Arc::new(AtomicUsize::new(0)),
            creation_failures: Arc::new(AtomicUsize::new(0)),
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
        // Check connection limit semaphore
        let _permit = self.semaphore.acquire().await
            .map_err(|_| PoolError::PoolExhausted)?;

        // Check creation rate limit semaphore
        let _creation_permit = self.creation_semaphore.acquire().await
            .map_err(|_| PoolError::PoolExhausted)?;

        // Create new connection
        let client = match self.factory.create().await {
            Ok(client) => client,
            Err(e) => {
                self.creation_failures.fetch_add(1, Ordering::SeqCst);
                return Err(PoolError::ConnectionFailed(e.to_string()));
            }
        };

        let conn = PooledConnection::new(client);
        let conn_id = conn.id;

        // Add to connections map (lock-free)
        self.connections.insert(conn_id, conn);

        // Add to available queue (lock-free)
        if self.available.push(conn_id).is_err() {
            // Queue is full - this shouldn't happen with proper sizing
            warn!("Available queue full when creating connection {}", conn_id);
            self.connections.remove(&conn_id);
            return Err(PoolError::PoolExhausted);
        }

        self.total_created.fetch_add(1, Ordering::SeqCst);
        debug!("Created new connection {} (total: {})", conn_id, self.connections.len());

        Ok(conn_id)
    }

    /// Acquire a session handle from the pool (optimized for high concurrency)
    pub async fn acquire(self: Arc<Self>) -> Result<SessionHandle, PoolError> {
        use tokio::time::{timeout, Instant};

        let start_time = Instant::now();

        // Check if shutting down
        if *self.is_shutting_down.lock().await {
            return Err(PoolError::ShuttingDown);
        }

        // Fast path: try to get an available connection (lock-free)
        if let Some(conn_id) = self.available.pop() {
            if let Some(mut conn_ref) = self.connections.get_mut(&conn_id) {
                // Check if connection is still valid
                if !conn_ref.is_expired(self.config.idle_timeout) && conn_ref.is_healthy {
                    conn_ref.mark_in_use();
                    let client = conn_ref.client.clone();
                    drop(conn_ref); // Release the reference early

                    self.total_acquired.fetch_add(1, Ordering::SeqCst);
                    self.current_active.fetch_add(1, Ordering::SeqCst);

                    debug!("Acquired connection {} from pool (fast path)", conn_id);
                    return Ok(SessionHandle::new(conn_id, client, Arc::clone(&self)));
                } else {
                    // Remove expired/unhealthy connection
                    self.connections.remove(&conn_id);
                    debug!("Removed expired/unhealthy connection {}", conn_id);
                }
            }
        }

        // Slow path: need to create a new connection or wait
        let total = self.connections.len();
        if total < self.config.max_connections {
            // Try to create a new connection with timeout
            match timeout(self.config.acquire_timeout, self.create_connection()).await {
                Ok(Ok(new_conn_id)) => {
                    // Immediately try to acquire the newly created connection
                    if let Some(mut conn_ref) = self.connections.get_mut(&new_conn_id) {
                        conn_ref.mark_in_use();
                        let client = conn_ref.client.clone();
                        drop(conn_ref);

                        self.total_acquired.fetch_add(1, Ordering::SeqCst);
                        self.current_active.fetch_add(1, Ordering::SeqCst);

                        debug!("Acquired newly created connection {} (slow path)", new_conn_id);
                        return Ok(SessionHandle::new(new_conn_id, client, Arc::clone(&self)));
                    }
                }
                Ok(Err(e)) => {
                    debug!("Failed to create new connection: {}", e);
                    return Err(e);
                }
                Err(_) => {
                    // Timeout
                    self.acquire_timeouts.fetch_add(1, Ordering::SeqCst);
                    warn!("Connection acquisition timed out after {:?}", start_time.elapsed());
                    return Err(PoolError::PoolExhausted);
                }
            }
        }

        // Pool is at capacity
        warn!("Connection pool exhausted (total: {}, active: {})",
              self.connections.len(), self.current_active.load(Ordering::SeqCst));
        self.acquire_timeouts.fetch_add(1, Ordering::SeqCst);
        Err(PoolError::PoolExhausted)
    }

    /// Release a connection back to the pool (optimized for high concurrency)
    async fn release_connection(&self, connection_id: Uuid) {
        if let Some(mut conn_ref) = self.connections.get_mut(&connection_id) {
            conn_ref.mark_available();
            drop(conn_ref); // Release the reference

            // Add back to available queue (lock-free)
            if self.available.push(connection_id).is_err() {
                // Queue is full - this shouldn't happen in normal operation
                warn!("Available queue full when releasing connection {}", connection_id);
            }

            self.total_released.fetch_add(1, Ordering::SeqCst);
            self.current_active.fetch_sub(1, Ordering::SeqCst);

            debug!("Released connection {} back to pool", connection_id);
        } else {
            warn!("Attempted to release unknown connection {}", connection_id);
        }
    }

    /// Maintain minimum pool size and clean up expired sessions
    async fn maintain_pool(self: Arc<Self>) {
        loop {
            if *self.is_shutting_down.lock().await {
                break;
            }

            let total_connections = self.connections.len();
            let _active = self.current_active.load(Ordering::SeqCst);

            if total_connections < self.config.min_connections {
                let needed = self.config.min_connections - total_connections;
                debug!("Pool below minimum, creating {} connections", needed);

                for _ in 0..needed {
                    if let Err(e) = self.create_connection().await {
                        warn!("Failed to maintain minimum pool size: {}", e);
                    }
                }
            }

            // Clean up expired and stuck connections
            let mut expired_ids = Vec::new();
            let mut stuck_ids = Vec::new();

            // Iterate through connections to find expired/stuck ones
            for entry in self.connections.iter() {
                let (id, conn) = entry.pair();
                // Remove idle expired connections
                if conn.is_expired(self.config.idle_timeout) && !conn.in_use {
                    expired_ids.push(*id);
                }
                // Detect stuck in-use connections
                else if conn.in_use && conn.last_used.elapsed() > self.config.max_session_duration {
                    stuck_ids.push(*id);
                    warn!("Detected stuck connection {} (in use for > 1 hour)", id);
                }
            }

            // Clean up expired connections (lock-free operations)
            for id in expired_ids {
                self.connections.remove(&id);
                // Note: ArrayQueue doesn't have retain, but expired connections
                // will be filtered out naturally during acquisition
                debug!("Removed expired connection {}", id);
            }

            // Force-release stuck connections
            for id in stuck_ids {
                if let Some(mut conn_ref) = self.connections.get_mut(&id) {
                    if conn_ref.in_use {
                        // Force release
                        conn_ref.mark_available();
                        conn_ref.is_healthy = false; // Mark as unhealthy for reconnection
                        self.current_active.fetch_sub(1, Ordering::SeqCst);
                        warn!("Force-released stuck connection {}", id);
                    }
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
            // Collect connections to health check (lock-free iteration)
            for entry in self.connections.iter() {
                let (id, conn) = entry.pair();
                // Only check connections that are available and haven't been checked recently
                if !conn.in_use && conn.last_used.elapsed() > Duration::from_secs(30) {
                    to_check.push((*id, conn.client.clone()));
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

            // Mark connections as unhealthy (lock-free operations)
            for id in &unhealthy_ids {
                if let Some(mut conn_ref) = self.connections.get_mut(id) {
                    conn_ref.is_healthy = false;
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

        // Check if connection is in use before attempting reconnect
        if let Some(conn_ref) = self.connections.get(&connection_id) {
            if conn_ref.in_use {
                warn!("Cannot reconnect in-use connection {}", connection_id);
                return;
            }
        }

        // Remove the old connection
        self.connections.remove(&connection_id);

        // Try to create a new connection
        let max_retries = 3;
        for attempt in 1..=max_retries {
            match self.factory.create().await {
                Ok(new_client) => {
                    let mut new_conn = PooledConnection::new(new_client);
                    new_conn.id = connection_id; // Reuse the same ID for tracking

                    // Add the new connection
                    self.connections.insert(connection_id, new_conn);

                    // Add to available queue (note: ArrayQueue doesn't have retain,
                    // but stale IDs will be filtered out during acquisition)
                    if self.available.push(connection_id).is_err() {
                        warn!("Available queue full during reconnect for connection {}", connection_id);
                    }

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
        self.connections.clear();
        // ArrayQueue doesn't have clear(), but we can drain it
        while self.available.pop().is_some() {
            // Drain the queue
        }

        info!("Connection pool shutdown complete");
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let total = self.connections.len();
        let active = self.current_active.load(Ordering::SeqCst);
        let available = self.available.len();

        PoolStats {
            available_connections: available,
            active_connections: active,
            total_connections: total,
            max_connections: self.config.max_connections,
            total_created: self.total_created.load(Ordering::SeqCst),
            total_acquired: self.total_acquired.load(Ordering::SeqCst),
            total_released: self.total_released.load(Ordering::SeqCst),
            acquire_timeouts: self.acquire_timeouts.load(Ordering::SeqCst),
            creation_failures: self.creation_failures.load(Ordering::SeqCst),
        }
    }

    /// Get detailed session information
    pub async fn get_sessions(&self) -> Vec<SessionInfo> {
        self.connections
            .iter()
            .map(|entry| {
                let (_, conn) = entry.pair();
                SessionInfo {
                    id: conn.id,
                    created_at: conn.created_at,
                    last_used: conn.last_used,
                    is_healthy: conn.is_healthy,
                    in_use: conn.in_use,
                }
            })
            .collect()
    }

    /// Forcefully disconnect a specific session
    pub async fn force_disconnect(&self, session_id: Uuid) -> bool {
        if let Some(mut conn_ref) = self.connections.get_mut(&session_id) {
            if conn_ref.in_use {
                conn_ref.mark_available();
                conn_ref.is_healthy = false;
                self.current_active.fetch_sub(1, Ordering::SeqCst);
                warn!("Force-disconnected session {}", session_id);
                return true;
            }
        }
        false
    }

    /// Clean up all disconnected or unhealthy sessions
    pub async fn cleanup_disconnected(&self) {
        let mut to_remove = Vec::new();

        // Collect unhealthy connections
        for entry in self.connections.iter() {
            let (id, conn) = entry.pair();
            if !conn.is_healthy && !conn.in_use {
                to_remove.push(*id);
            }
        }

        // Remove unhealthy connections (lock-free operations)
        for id in to_remove {
            self.connections.remove(&id);
            // Note: ArrayQueue doesn't have retain, but stale IDs
            // will be filtered out naturally during acquisition
            info!("Cleaned up disconnected session {}", id);
        }
    }

    /// Check if a session is still valid
    pub async fn is_session_valid(&self, session_id: Uuid) -> bool {
        if let Some(conn_ref) = self.connections.get(&session_id) {
            conn_ref.is_healthy &&
            !conn_ref.is_expired(self.config.idle_timeout) &&
            (conn_ref.in_use || conn_ref.last_used.elapsed() < self.config.max_session_duration)
        } else {
            false
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
    pub total_created: usize,
    pub total_acquired: usize,
    pub total_released: usize,
    pub acquire_timeouts: usize,
    pub creation_failures: usize,
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