// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use crossbeam::queue::ArrayQueue;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tokio::time::sleep;
use uuid::Uuid;

use crate::imap::{AsyncImapSessionWrapper, ImapClient, ImapError};

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
    /// Minimum number of idle connections to keep warm. These are kept alive
    /// with NOOP health checks and are never recycled just for being idle.
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Time before a *surplus* idle connection (one above `min_connections`)
    /// is closed. Also the minimum "proof of life" window: a connection that
    /// has neither been used nor passed a NOOP health check within
    /// `max(idle_timeout, 3 * health_check_interval)` is treated as stale.
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
            // Default 1: every extra warm connection is another IMAP session the
            // server has to hold open. Bursts grow the pool on demand.
            min_connections: std::env::var("POOL_MIN_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            max_connections: std::env::var("MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            idle_timeout: Duration::from_secs(
                std::env::var("POOL_IDLE_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60),
            ),
            health_check_interval: Duration::from_secs(
                std::env::var("POOL_HEALTH_CHECK_INTERVAL_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30),
            ),
            acquire_timeout: Duration::from_secs(
                std::env::var("POOL_ACQUIRE_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5),
            ),
            max_session_duration: Duration::from_secs(
                std::env::var("POOL_MAX_SESSION_DURATION_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(300),
            ),
            max_concurrent_creations: std::env::var("POOL_MAX_CONCURRENT_CREATIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
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
    /// Last time this connection passed a NOOP health check
    last_validated: Instant,
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
            last_validated: now,
            is_healthy: true,
            in_use: false,
        }
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    /// Idle (unused) for longer than `idle_timeout`. Only surplus connections
    /// above `min_connections` are closed for this reason.
    fn is_expired(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }

    /// Most recent evidence that the connection works: last use or last
    /// successful NOOP, whichever is newer.
    fn last_alive(&self) -> Instant {
        self.last_used.max(self.last_validated)
    }

    /// No use and no successful health check within `window`.
    fn is_stale(&self, window: Duration) -> bool {
        is_stale_at(self.last_used, self.last_validated, window, Instant::now())
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
    fn new(
        connection_id: Uuid,
        client: Arc<ImapClient<AsyncImapSessionWrapper>>,
        pool: Arc<ConnectionPool>,
    ) -> Self {
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
        )
        .await?;
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

    /// How long an idle connection may go without use or a passing NOOP
    /// before it is considered stale and replaced.
    fn liveness_window(&self) -> Duration {
        self.config
            .idle_timeout
            .max(self.config.health_check_interval * 3)
    }

    /// Create a new connection and add to pool
    async fn create_connection(&self) -> Result<Uuid, PoolError> {
        // Check connection limit semaphore
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| PoolError::PoolExhausted)?;

        // Check creation rate limit semaphore
        let _creation_permit = self
            .creation_semaphore
            .acquire()
            .await
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
        debug!(
            "Created new connection {} (total: {})",
            conn_id,
            self.connections.len()
        );

        Ok(conn_id)
    }

    /// Remove a connection from the pool with proper cleanup (logout).
    /// Logout closes the server-side IMAP session cleanly; the client's read
    /// buffers are freed when the last `Arc` to the client is dropped.
    async fn remove_connection_with_logout(&self, conn_id: &Uuid) {
        if let Some((_, conn)) = self.connections.remove(conn_id) {
            // Log out so the server closes the session cleanly
            if let Err(e) = conn.client.logout().await {
                debug!("Logout failed for connection {} during cleanup (may already be disconnected): {}", conn_id, e);
            } else {
                debug!(
                    "Successfully logged out connection {} during cleanup",
                    conn_id
                );
            }
        }
    }

    /// Remove multiple connections with proper cleanup (spawns tasks for parallel logout)
    fn remove_connections_with_logout(&self, conn_ids: Vec<Uuid>) {
        for conn_id in conn_ids {
            if let Some((_, conn)) = self.connections.remove(&conn_id) {
                // Spawn logout task to avoid blocking the maintenance loop
                tokio::spawn(async move {
                    if let Err(e) = conn.client.logout().await {
                        debug!(
                            "Logout failed for connection {} during cleanup: {}",
                            conn_id, e
                        );
                    } else {
                        debug!(
                            "Successfully logged out connection {} during cleanup",
                            conn_id
                        );
                    }
                    // Connection (and its read buffer) is dropped here
                });
            }
        }
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
                if !conn_ref.is_stale(self.liveness_window()) && conn_ref.is_healthy {
                    conn_ref.mark_in_use();
                    let client = conn_ref.client.clone();
                    drop(conn_ref); // Release the reference early

                    self.total_acquired.fetch_add(1, Ordering::SeqCst);
                    self.current_active.fetch_add(1, Ordering::SeqCst);

                    debug!("Acquired connection {} from pool (fast path)", conn_id);
                    return Ok(SessionHandle::new(conn_id, client, Arc::clone(&self)));
                } else {
                    // Remove stale/unhealthy connection with logout (non-blocking)
                    if let Some((_, conn)) = self.connections.remove(&conn_id) {
                        debug!(
                            "Removed stale/unhealthy connection {}, spawning logout task",
                            conn_id
                        );
                        tokio::spawn(async move {
                            if let Err(e) = conn.client.logout().await {
                                debug!("Logout failed for expired connection {}: {}", conn_id, e);
                            }
                        });
                    }
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

                        debug!(
                            "Acquired newly created connection {} (slow path)",
                            new_conn_id
                        );
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
                    warn!(
                        "Connection acquisition timed out after {:?}",
                        start_time.elapsed()
                    );
                    return Err(PoolError::PoolExhausted);
                }
            }
        }

        // Pool is at capacity
        warn!(
            "Connection pool exhausted (total: {}, active: {})",
            self.connections.len(),
            self.current_active.load(Ordering::SeqCst)
        );
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
                warn!(
                    "Available queue full when releasing connection {}",
                    connection_id
                );
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

            // Idle connections are only closed when they are surplus (above
            // min_connections) or stale (no use and no passing NOOP recently).
            // Healthy warm connections are kept: closing and reopening them was
            // creating thousands of IMAP sessions per day.
            let liveness_window = self.liveness_window();
            let mut idle_candidates = Vec::new();
            for entry in self.connections.iter() {
                let (id, conn) = entry.pair();
                if conn.in_use {
                    // Detect stuck in-use connections
                    if conn.last_used.elapsed() > self.config.max_session_duration {
                        stuck_ids.push(*id);
                        warn!(
                            "Detected stuck connection {} (in use for > {:?})",
                            id, self.config.max_session_duration
                        );
                    }
                } else if conn.is_stale(liveness_window) {
                    expired_ids.push(*id);
                } else if conn.is_expired(self.config.idle_timeout) {
                    idle_candidates.push((*id, conn.last_used));
                }
            }
            let remaining = self.connections.len().saturating_sub(expired_ids.len());
            expired_ids.extend(select_surplus_idle(
                idle_candidates,
                remaining,
                self.config.min_connections,
            ));

            // Clean up expired connections with proper logout
            if !expired_ids.is_empty() {
                debug!(
                    "Cleaning up {} expired connections with logout",
                    expired_ids.len()
                );
                self.remove_connections_with_logout(expired_ids);
            }

            // Periodically rebuild the available queue to remove stale IDs
            // This prevents unbounded queue growth from accumulating stale UUIDs
            let valid_ids: Vec<Uuid> = self
                .connections
                .iter()
                .filter(|entry| {
                    let (_, conn) = entry.pair();
                    !conn.in_use && conn.is_healthy && !conn.is_stale(liveness_window)
                })
                .map(|entry| *entry.key())
                .collect();

            // Drain the old queue
            while self.available.pop().is_some() {
                // Drain all stale IDs
            }

            // Rebuild with only valid IDs
            for id in valid_ids {
                if self.available.push(id).is_err() {
                    warn!("Queue full during rebuild, skipping connection {}", id);
                }
            }
            debug!(
                "Rebuilt available queue with {} valid connections",
                self.available.len()
            );

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
            // Re-check anything idle that hasn't shown signs of life for about
            // half an interval (so each idle connection is NOOP'd once per loop).
            let recheck_after = self.config.health_check_interval / 2;
            for entry in self.connections.iter() {
                let (id, conn) = entry.pair();
                if !conn.in_use && conn.last_alive().elapsed() > recheck_after {
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
                    // Passing NOOP counts as proof of life, so the connection
                    // is kept instead of being recycled as "idle".
                    if let Some(mut conn_ref) = self.connections.get_mut(&id) {
                        conn_ref.last_validated = Instant::now();
                    }
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

        // Remove the old connection with proper logout
        self.remove_connection_with_logout(&connection_id).await;

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
                        warn!(
                            "Available queue full during reconnect for connection {}",
                            connection_id
                        );
                    }

                    info!(
                        "Successfully reconnected connection {} on attempt {}",
                        connection_id, attempt
                    );
                    return;
                }
                Err(e) => {
                    warn!(
                        "Reconnection attempt {} failed for connection {}: {}",
                        attempt, connection_id, e
                    );
                    if attempt < max_retries {
                        sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    }
                }
            }
        }

        error!(
            "Failed to reconnect connection {} after {} attempts",
            connection_id, max_retries
        );
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

        // Remove unhealthy connections with proper logout
        if !to_remove.is_empty() {
            info!(
                "Cleaning up {} unhealthy connections with logout",
                to_remove.len()
            );
            self.remove_connections_with_logout(to_remove);
        }
    }

    /// Check if a session is still valid
    pub async fn is_session_valid(&self, session_id: Uuid) -> bool {
        if let Some(conn_ref) = self.connections.get(&session_id) {
            conn_ref.is_healthy
                && !conn_ref.is_stale(self.liveness_window())
                && (conn_ref.in_use
                    || conn_ref.last_used.elapsed() < self.config.max_session_duration)
        } else {
            false
        }
    }
}

/// True if neither use nor a passing health check happened within `window`
/// before `now`.
fn is_stale_at(
    last_used: Instant,
    last_validated: Instant,
    window: Duration,
    now: Instant,
) -> bool {
    now.saturating_duration_since(last_used.max(last_validated)) > window
}

/// Pick which idle connections to close so that at least `min_connections`
/// remain. `candidates` are idle-expired connections as `(id, last_used)`;
/// `total` is the current connection count. Oldest-idle connections go first.
fn select_surplus_idle(
    mut candidates: Vec<(Uuid, Instant)>,
    total: usize,
    min_connections: usize,
) -> Vec<Uuid> {
    let surplus = total.saturating_sub(min_connections);
    candidates.sort_by_key(|(_, last_used)| *last_used);
    candidates
        .into_iter()
        .take(surplus)
        .map(|(id, _)| id)
        .collect()
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
        assert_eq!(stats.max_connections, 50); // Updated to match new memory-optimized default
    }

    #[test]
    fn test_default_min_connections_is_one() {
        if std::env::var("POOL_MIN_CONNECTIONS").is_err() {
            assert_eq!(PoolConfig::default().min_connections, 1);
        }
    }

    #[test]
    fn test_warm_connections_are_not_trimmed() {
        // Regression: idle connections at or below min_connections used to be
        // closed every idle_timeout and immediately recreated (~5,500
        // IMAP sessions/day with min=5, idle_timeout=60s).
        let now = Instant::now();
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        let candidates: Vec<(Uuid, Instant)> = ids.iter().map(|id| (*id, now)).collect();

        // At the minimum: nothing is closed.
        assert!(select_surplus_idle(candidates.clone(), 3, 3).is_empty());
        // Below the minimum: nothing is closed.
        assert!(select_surplus_idle(candidates.clone(), 3, 5).is_empty());
        // One above the minimum: exactly one is closed.
        assert_eq!(select_surplus_idle(candidates, 3, 2).len(), 1);
    }

    #[test]
    fn test_surplus_trim_closes_oldest_first() {
        let oldest = Instant::now();
        let older = oldest + Duration::from_secs(300);
        let newest = oldest + Duration::from_secs(600);
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let picked = select_surplus_idle(vec![(a, newest), (b, oldest), (c, older)], 3, 1);
        assert_eq!(picked, vec![b, c]);
    }

    #[test]
    fn test_passing_noop_keeps_idle_connection_fresh() {
        let window = Duration::from_secs(90);
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(3600);
        // Unused for an hour and never validated: stale.
        assert!(is_stale_at(t0, t0, window, now));
        // Unused for an hour but passed NOOP 30s ago: kept.
        let validated = now - Duration::from_secs(30);
        assert!(!is_stale_at(t0, validated, window, now));
    }

    #[test]
    fn test_byte_pool_not_in_dependency_tree() {
        // async-imap < 0.9.1 used a process-global byte-pool that never
        // freed returned buffers, so every IMAP session leaked its read
        // buffers (17 GB after 11 days in production). Guard against it
        // coming back through a downgrade or another crate.
        let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
        assert!(
            !lock.contains("name = \"byte-pool\""),
            "byte-pool is back in Cargo.lock; see docs/MEMORY_LEAK_ANALYSIS.md"
        );
    }
}
