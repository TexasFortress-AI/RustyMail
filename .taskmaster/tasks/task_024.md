# Task ID: 24

**Title:** Remove hardcoded test credentials and require configured API keys

**Status:** done

**Dependencies:** 22 ✓, 23 ✓

**Priority:** high

**Description:** Remove all hardcoded test API keys and credentials from the codebase, require explicit configuration of all API keys, add key expiration support, and update documentation for secure key generation.

**Details:**

Fix critical security vulnerabilities in API key management by removing all hardcoded credentials and test keys:

1) **Remove hardcoded test key initialization in src/api/auth.rs**:
   - Delete or comment out the ApiKeyStore::init_with_defaults() method that seeds a test key with Admin scope at startup
   - Ensure no API keys are automatically created when the application starts
   - Modify the initialization logic to require explicit key configuration through environment variables or secure configuration files

2) **Remove test credentials from .env.example**:
   - Remove the line `RUSTYMAIL_API_KEY=test-rustymail-key-2024` from .env.example
   - Replace with a placeholder like `RUSTYMAIL_API_KEY=your-secure-api-key-here`
   - Add comments explaining that users must generate their own secure API keys

3) **Implement API key expiration support**:
   - Add an `expires_at` field to the API key storage structure (likely in ApiKeyStore)
   - Modify the key validation logic to check expiration timestamps
   - Return 401 Unauthorized for expired keys with appropriate error messages
   - Consider adding a configurable default expiration period (e.g., 90 days)

4) **Remove any hardcoded IMAP credentials**:
   - Search the codebase for any hardcoded IMAP usernames, passwords, or server configurations
   - Ensure all IMAP credentials must be provided through secure configuration
   - Update any test configurations to use environment variables instead

5) **Add secure key generation documentation**:
   - Create a new section in the README or a separate SECURITY.md file
   - Document how to generate cryptographically secure API keys (e.g., using openssl rand -hex 32)
   - Explain the importance of key rotation and expiration
   - Provide examples of secure key storage practices
   - Document the required scopes and permissions for different API operations

6) **Update application startup logic**:
   - Add validation to ensure required API keys are configured before the application starts
   - Provide clear error messages if required keys are missing
   - Consider implementing a setup wizard or initialization script for first-time configuration

**Test Strategy:**

Verify the security improvements with comprehensive testing:

1) **Test removal of hardcoded keys**:
   - Start the application with a clean environment (no API keys configured)
   - Verify the application refuses to start or enters a safe mode without any pre-configured keys
   - Confirm no test keys are accessible through the API

2) **Test API key expiration**:
   - Create an API key with a short expiration time (e.g., 1 minute in the future)
   - Make successful API calls with the key
   - Wait for the key to expire
   - Verify subsequent API calls return 401 Unauthorized with an "expired key" error message

3) **Test IMAP credential requirements**:
   - Attempt to use IMAP functionality without configuring credentials
   - Verify appropriate error messages are returned
   - Configure valid IMAP credentials through environment variables
   - Confirm IMAP functionality works correctly with configured credentials

4) **Test .env.example changes**:
   - Copy .env.example to .env
   - Verify the application doesn't start with placeholder values
   - Replace placeholders with valid keys and confirm successful startup

5) **Security audit**:
   - Search the entire codebase for strings like "test", "default", "admin" in authentication contexts
   - Verify no hardcoded credentials remain in any source files
   - Check that all authentication-related configuration comes from environment variables or secure config files

6) **Documentation verification**:
   - Follow the new secure key generation documentation to create API keys
   - Verify the generated keys work correctly with the application
   - Confirm all security best practices are clearly explained
