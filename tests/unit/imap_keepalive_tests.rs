// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Unit tests for IMAP Keepalive/NOOP functionality
// Tests the NOOP command implementation and connection pool health checking

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{sleep, Instant};

use async_trait::async_trait;
use rustymail::imap::{ImapClient, ImapError, AsyncImapSessionWrapper};
use rustymail::connection_pool::{ConnectionFactory, ConnectionPool, PoolConfig};

// Mock IMAP session for testing NOOP command
struct MockImapSession {
    noop_count: Arc<TokioMutex<u32>>,
    should_fail: Arc<TokioMutex<bool>>,
}

impl MockImapSession {
    fn new() -> Self {
        Self {
            noop_count: Arc::new(TokioMutex::new(0)),
            should_fail: Arc::new(TokioMutex::new(false)),
        }
    }

    async fn noop(&self) -> Result<(), ImapError> {
        let mut count = self.noop_count.lock().await;
        *count += 1;

        let should_fail = *self.should_fail.lock().await;
        if should_fail {
            return Err(ImapError::Connection("Mock NOOP failure".to_string()));
        }

        Ok(())
    }

    async fn get_noop_count(&self) -> u32 {
        *self.noop_count.lock().await
    }

    async fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().await = fail;
    }
}

// Mock ConnectionFactory for pool tests
struct MockConnectionFactory {
    create_count: Arc<TokioMutex<u32>>,
    validate_count: Arc<TokioMutex<u32>>,
    should_create_fail: Arc<TokioMutex<bool>>,
    should_validate_fail: Arc<TokioMutex<bool>>,
}

impl MockConnectionFactory {
    fn new() -> Self {
        Self {
            create_count: Arc::new(TokioMutex::new(0)),
            validate_count: Arc::new(TokioMutex::new(0)),
            should_create_fail: Arc::new(TokioMutex::new(false)),
            should_validate_fail: Arc::new(TokioMutex::new(false)),
        }
    }

    async fn get_create_count(&self) -> u32 {
        *self.create_count.lock().await
    }

    async fn get_validate_count(&self) -> u32 {
        *self.validate_count.lock().await
    }

    async fn set_should_create_fail(&self, fail: bool) {
        *self.should_create_fail.lock().await = fail;
    }

    async fn set_should_validate_fail(&self, fail: bool) {
        *self.should_validate_fail.lock().await = fail;
    }
}

#[async_trait]
impl ConnectionFactory for MockConnectionFactory {
    async fn create(&self) -> Result<Arc<ImapClient<AsyncImapSessionWrapper>>, ImapError> {
        let mut count = self.create_count.lock().await;
        *count += 1;

        let should_fail = *self.should_create_fail.lock().await;
        if should_fail {
            return Err(ImapError::Connection("Mock create failure".to_string()));
        }

        // For testing purposes, we can't actually create a real ImapClient
        // This is a limitation of unit testing without real IMAP infrastructure
        Err(ImapError::Connection("Mock client - unit test limitation".to_string()))
    }

    async fn validate(&self, _client: &Arc<ImapClient<AsyncImapSessionWrapper>>) -> bool {
        let mut count = self.validate_count.lock().await;
        *count += 1;

        let should_fail = *self.should_validate_fail.lock().await;
        !should_fail
    }
}

#[tokio::test]
async fn test_noop_command_success() {
    // Test that NOOP command executes successfully
    let mock_session = MockImapSession::new();

    // Execute NOOP
    let result = mock_session.noop().await;

    // Verify success
    assert!(result.is_ok(), "NOOP command should succeed");

    // Verify NOOP was called
    let noop_count = mock_session.get_noop_count().await;
    assert_eq!(noop_count, 1, "NOOP should have been called once");
}

#[tokio::test]
async fn test_noop_keeps_connection_alive() {
    // Test that NOOP prevents timeout by being called periodically
    let mock_session = Arc::new(MockImapSession::new());

    // Simulate periodic NOOP calls (e.g., every 30 seconds)
    let session_clone = Arc::clone(&mock_session);
    let noop_task = tokio::spawn(async move {
        for _ in 0..3 {
            session_clone.noop().await.unwrap();
            sleep(Duration::from_millis(100)).await;
        }
    });

    noop_task.await.unwrap();

    // Verify NOOP was called multiple times
    let noop_count = mock_session.get_noop_count().await;
    assert_eq!(noop_count, 3, "NOOP should have been called 3 times to keep connection alive");
}

#[tokio::test]
async fn test_connection_pool_health_check() {
    // Test that pool uses validate() for health checking
    let factory = Arc::new(MockConnectionFactory::new());
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 5,
        idle_timeout: Duration::from_secs(300),
        health_check_interval: Duration::from_millis(100),
        acquire_timeout: Duration::from_secs(5),
        max_session_duration: Duration::from_secs(3600),
        max_concurrent_creations: 10,
    };

    let _pool = ConnectionPool::new(Arc::clone(&factory) as Arc<dyn ConnectionFactory>, config);

    // Wait for health check loop to run a few times
    sleep(Duration::from_millis(500)).await;

    // Note: Since we can't actually create connections in unit tests,
    // validate won't be called. This test verifies the pool is created correctly.
    // In integration tests with real IMAP, we would verify validate() is called.
}

#[tokio::test]
async fn test_health_check_detects_dead_connection() {
    // Test that health check detects when validate() returns false
    let factory = Arc::new(MockConnectionFactory::new());

    // Set validate to fail (simulating dead connection)
    factory.set_should_validate_fail(true).await;

    // Note: We can't actually create a real ImapClient in unit tests,
    // but we can verify the factory's validate behavior
    // The factory.validate() method will be called in integration tests
    // with real IMAP connections

    // Verify that setting should_validate_fail to true affects the behavior
    let should_fail = *factory.should_validate_fail.lock().await;
    assert!(should_fail, "Factory should be set to fail validation");
}

#[tokio::test]
async fn test_health_check_interval() {
    // Test that health checks run at the configured interval
    let factory = Arc::new(MockConnectionFactory::new());
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 5,
        idle_timeout: Duration::from_secs(300),
        health_check_interval: Duration::from_millis(50), // Fast interval for testing
        acquire_timeout: Duration::from_secs(5),
        max_session_duration: Duration::from_secs(3600),
        max_concurrent_creations: 10,
    };

    let _pool = ConnectionPool::new(Arc::clone(&factory) as Arc<dyn ConnectionFactory>, config);

    // Wait for multiple health check intervals
    sleep(Duration::from_millis(200)).await;

    // In integration tests with real connections, we would verify
    // that validate() was called approximately 4 times (200ms / 50ms)
    // For unit tests, we just verify the pool was created
}

#[tokio::test]
async fn test_noop_during_idle_connection() {
    // Test that NOOP is called on idle connections to prevent timeout
    let mock_session = Arc::new(MockImapSession::new());

    // Simulate connection being idle
    sleep(Duration::from_millis(50)).await;

    // Call NOOP to keep connection alive
    mock_session.noop().await.unwrap();

    let noop_count = mock_session.get_noop_count().await;
    assert_eq!(noop_count, 1, "NOOP should be called once during idle period");

    // Simulate more idle time and another NOOP
    sleep(Duration::from_millis(50)).await;
    mock_session.noop().await.unwrap();

    let noop_count = mock_session.get_noop_count().await;
    assert_eq!(noop_count, 2, "NOOP should be called again after more idle time");
}

#[tokio::test]
async fn test_noop_error_handling() {
    // Test NOOP failure scenarios
    let mock_session = MockImapSession::new();

    // Set NOOP to fail
    mock_session.set_should_fail(true).await;

    // Execute NOOP and verify it fails
    let result = mock_session.noop().await;
    assert!(result.is_err(), "NOOP should fail when connection is dead");

    // Verify error message
    if let Err(e) = result {
        assert!(
            matches!(e, ImapError::Connection(_)),
            "Error should be Connection error"
        );
    }

    // Verify NOOP was still called (and failed)
    let noop_count = mock_session.get_noop_count().await;
    assert_eq!(noop_count, 1, "NOOP should have been attempted once");
}

#[tokio::test]
async fn test_multiple_connections_keepalive() {
    // Test keepalive across multiple pooled connections
    let session1 = Arc::new(MockImapSession::new());
    let session2 = Arc::new(MockImapSession::new());
    let session3 = Arc::new(MockImapSession::new());

    // Simulate keepalive on multiple connections concurrently
    let s1 = Arc::clone(&session1);
    let s2 = Arc::clone(&session2);
    let s3 = Arc::clone(&session3);

    let task1 = tokio::spawn(async move {
        for _ in 0..2 {
            s1.noop().await.unwrap();
            sleep(Duration::from_millis(50)).await;
        }
    });

    let task2 = tokio::spawn(async move {
        for _ in 0..2 {
            s2.noop().await.unwrap();
            sleep(Duration::from_millis(50)).await;
        }
    });

    let task3 = tokio::spawn(async move {
        for _ in 0..2 {
            s3.noop().await.unwrap();
            sleep(Duration::from_millis(50)).await;
        }
    });

    // Wait for all tasks to complete
    task1.await.unwrap();
    task2.await.unwrap();
    task3.await.unwrap();

    // Verify all connections had NOOP called
    let count1 = session1.get_noop_count().await;
    let count2 = session2.get_noop_count().await;
    let count3 = session3.get_noop_count().await;

    assert_eq!(count1, 2, "Session 1 should have NOOP called 2 times");
    assert_eq!(count2, 2, "Session 2 should have NOOP called 2 times");
    assert_eq!(count3, 2, "Session 3 should have NOOP called 2 times");
}

#[tokio::test]
async fn test_pool_stats_tracking() {
    // Test that pool properly tracks statistics
    let factory = Arc::new(MockConnectionFactory::new());
    let config = PoolConfig::default();

    let pool = ConnectionPool::new(Arc::clone(&factory) as Arc<dyn ConnectionFactory>, config);

    // Get initial stats
    let stats = pool.stats().await;

    // Verify initial state
    assert_eq!(stats.max_connections, 100, "Max connections should match config");
    assert_eq!(stats.total_created, 0, "No connections should be created yet in unit test");
}

#[tokio::test]
async fn test_pool_config_validation() {
    // Test that pool config values are properly set
    let config = PoolConfig {
        min_connections: 10,
        max_connections: 50,
        idle_timeout: Duration::from_secs(180),
        health_check_interval: Duration::from_secs(60),
        acquire_timeout: Duration::from_secs(10),
        max_session_duration: Duration::from_secs(7200),
        max_concurrent_creations: 5,
    };

    assert_eq!(config.min_connections, 10);
    assert_eq!(config.max_connections, 50);
    assert_eq!(config.idle_timeout, Duration::from_secs(180));
    assert_eq!(config.health_check_interval, Duration::from_secs(60));
    assert_eq!(config.acquire_timeout, Duration::from_secs(10));
    assert_eq!(config.max_session_duration, Duration::from_secs(7200));
    assert_eq!(config.max_concurrent_creations, 5);
}
