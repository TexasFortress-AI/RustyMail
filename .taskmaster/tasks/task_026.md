# Task ID: 26

**Title:** Implement encryption for stored credentials

**Status:** done

**Dependencies:** 24 ✓

**Priority:** medium

**Description:** Add encryption at rest for all sensitive credentials including IMAP/SMTP passwords in database and JSON config files, with support for application-level encryption or external KMS integration.

**Details:**

Implement a comprehensive encryption solution for all stored credentials:

1) Create encryption module at src/dashboard/services/security/encryption.rs:
   - Define CredentialEncryption trait with encrypt() and decrypt() methods
   - Implement ApplicationLevelEncryption using AES-256-GCM with master key from ENCRYPTION_MASTER_KEY env var
   - Implement KmsEncryption for AWS KMS/Azure Key Vault integration (configurable via ENCRYPTION_PROVIDER env var)
   - Add EncryptionService that selects provider based on configuration

2) Update database schema with migration 005_add_credential_encryption.sql:
   ```sql
   ALTER TABLE email_accounts 
   ADD COLUMN password_encrypted BYTEA,
   ADD COLUMN encryption_metadata JSONB;
   
   ALTER TABLE ai_model_configurations
   ADD COLUMN api_key_encrypted BYTEA,
   ADD COLUMN encryption_metadata JSONB;
   ```

3) Modify src/dashboard/models/email_account.rs:
   - Add password_encrypted and encryption_metadata fields
   - Update create() and update() methods to encrypt password before storage
   - Modify get_password() to decrypt on retrieval
   - Keep backward compatibility during migration

4) Update src/dashboard/models/ai_model_configuration.rs similarly for api_key field

5) Create migration script src/dashboard/services/security/migrate_credentials.rs:
   - Scan all email_accounts and ai_model_configurations records
   - For each plaintext credential, encrypt and store in new columns
   - Verify decryption works correctly
   - Once verified, null out plaintext columns

6) Update JSON config handling in src/config/mod.rs:
   - Detect plaintext credentials in config files
   - Encrypt and rewrite config with encrypted values
   - Add encryption_metadata to track encryption method

7) Add key rotation support:
   - Implement rotate_encryption_key() method
   - Re-encrypt all credentials with new key
   - Update encryption_metadata with rotation timestamp

**Test Strategy:**

Verify encryption implementation with comprehensive testing:

1) Unit tests for encryption module:
   - Test AES-256-GCM encryption/decryption with known test vectors
   - Verify different length passwords encrypt correctly
   - Test error handling for invalid keys or corrupted data
   - Mock KMS integration tests

2) Integration tests for database operations:
   - Create email account with password, verify it's stored encrypted
   - Retrieve account and confirm password decrypts correctly
   - Test migration script on test data with mix of plaintext/encrypted records
   - Verify AI model API keys are encrypted similarly

3) End-to-end testing:
   - Set ENCRYPTION_MASTER_KEY and restart application
   - Create new email account via API/UI
   - Query database directly to confirm password_encrypted is populated and password is null
   - Use account for IMAP/SMTP operations to verify decryption works
   - Test with missing ENCRYPTION_MASTER_KEY to ensure proper error handling

4) Security validation:
   - Verify encrypted values are different even for same plaintext (due to random IV)
   - Confirm encryption metadata includes algorithm version for future compatibility
   - Test key rotation functionality with multiple credentials
