/// Test helpers module
/// Provides common utilities and environment setup for tests
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment with required environment variables
/// This should be called at the start of any test that needs configuration
pub fn setup_test_env() {
    INIT.call_once(|| {
        // Load from .env.test if it exists, otherwise use defaults
        dotenv::from_filename(".env.test").ok();

        // Set required environment variables with test defaults if not already set
        set_if_unset("REST_HOST", "127.0.0.1");
        set_if_unset("REST_PORT", "9437");
        set_if_unset("SSE_HOST", "127.0.0.1");
        set_if_unset("SSE_PORT", "9438");
        set_if_unset("DASHBOARD_PORT", "9439");
        set_if_unset("RUSTYMAIL_API_KEY", "test-rustymail-key-2024");
        set_if_unset("CACHE_DATABASE_URL", "sqlite::memory:");
        set_if_unset("MCP_BACKEND_URL", "http://localhost:9437/mcp");
        set_if_unset("MCP_TIMEOUT", "30");

        // IMAP test configuration (mock adapter)
        set_if_unset("IMAP_ADAPTER", "mock");
        set_if_unset("IMAP_HOST", "localhost");
        set_if_unset("IMAP_PORT", "143");
        set_if_unset("IMAP_USER", "test@example.com");
        set_if_unset("IMAP_PASS", "testpass");
    });
}

/// Set environment variable only if not already set
fn set_if_unset(key: &str, value: &str) {
    if std::env::var(key).is_err() {
        std::env::set_var(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_test_env() {
        setup_test_env();

        // Verify required variables are set
        assert!(std::env::var("REST_HOST").is_ok());
        assert!(std::env::var("REST_PORT").is_ok());
        assert!(std::env::var("RUSTYMAIL_API_KEY").is_ok());
    }
}
