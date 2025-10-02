# Multi-Account Database Schema Design

## Overview
This document describes the database schema design for supporting multiple email accounts in RustyMail.

## Design Principles

1. **Backward Compatibility**: Existing single-account installations should migrate seamlessly
2. **Security**: Credentials stored with encryption support planned
3. **Extensibility**: Support for future OAuth2 and additional providers
4. **Isolation**: Each account's data is properly isolated via foreign keys
5. **Scalability**: Schema supports multiple accounts per user efficiently

## Schema Components

### 1. Accounts Table (`accounts`)

**Purpose**: Store email account configurations and credentials

**Key Fields**:
- `id`: Primary key
- `account_name`: User-friendly display name (e.g., "Work Gmail")
- `email_address`: Full email address (unique identifier)
- `provider_type`: Auto-detected provider ('gmail', 'outlook', 'yahoo', 'other')
- `provider_metadata`: JSON field for provider-specific configuration
- `imap_*`: IMAP connection settings
- `smtp_*`: SMTP connection settings (for future email sending)
- `oauth_*`: OAuth2 token storage (for future Gmail/Outlook integration)
- `is_active`: Whether account is enabled
- `is_default`: One account marked as default (unique index enforces this)
- `last_connected`: Connection health tracking
- `last_error`: Error state for UI display

**Security Considerations**:
- `imap_pass` and `smtp_pass` currently stored as plaintext
- TODO: Implement encryption at rest (e.g., using SQLite encryption or application-level encryption)
- OAuth tokens should be encrypted similarly

**Indexes**:
- Unique index on `account_name`
- Unique index on `email_address`
- Unique partial index on `is_default` WHERE `is_default = TRUE` (ensures only one default)

### 2. Folders Table (Modified)

**Changes**: Added `account_id` foreign key

**Migration Strategy**:
1. Create new `folders_new` table with `account_id` column
2. Insert default account record
3. Migrate existing folders to new table with `account_id = 1`
4. Drop old table and rename new one
5. Recreate indexes and triggers

**Key Constraint**:
- `UNIQUE(account_id, name)`: Folder names are unique per account

### 3. Emails Table (No Changes)

**Relationship**:
- `emails.folder_id → folders.id → folders.account_id → accounts.id`
- No direct modification needed
- Account isolation is maintained through the folder relationship

### 4. Account Sessions Table (`account_sessions`)

**Purpose**: Track active IMAP connections per account

**Key Fields**:
- `account_id`: Links to accounts table
- `session_token`: Unique identifier for connection pool
- `connection_state`: 'connected', 'disconnected', 'error'
- `last_activity`: Health monitoring

**Use Cases**:
- Connection pool management per account
- Detecting stale connections
- UI status indicators

### 5. Provider Templates Table (`provider_templates`)

**Purpose**: Auto-configuration data for common providers

**Pre-populated Providers**:
- Gmail (supports OAuth)
- Outlook/Hotmail (supports OAuth)
- Yahoo Mail
- iCloud Mail
- Fastmail

**Key Fields**:
- `provider_type`: Unique identifier
- `domain_patterns`: JSON array for email domain matching
- `imap_*` / `smtp_*`: Connection settings
- `supports_oauth`: Flag for OAuth support
- `oauth_provider`: Provider identifier for OAuth flow

**Auto-Configuration Flow**:
1. User enters email address
2. Extract domain from email
3. Match against `domain_patterns` in provider_templates
4. Pre-fill connection settings
5. User only needs to provide credentials

## Data Relationships

```
accounts (1) ──┬── (N) folders
               │       │
               │       └── (N) emails
               │              │
               │              └── (N) attachments
               │
               └── (N) account_sessions

provider_templates (reference data, no FK relationships)
```

## Migration Path

### For New Installations
- Schema created with multi-account support from start
- No default account created automatically
- User must add first account through UI

### For Existing Installations
1. Migration creates default account with placeholder values
2. Existing folders assigned to default account
3. User should update default account credentials in UI
4. Email/folder data preserved without re-sync

## Security Considerations

### Current Implementation
- Passwords stored in plaintext in SQLite
- Suitable for development and personal use
- Not suitable for production/hosted environments

### Recommended Future Enhancements
1. **SQLite Encryption**: Use SQLCipher or similar for database-level encryption
2. **Application-Level Encryption**: Encrypt credentials before storage using a master key
3. **OAuth2 Priority**: For Gmail/Outlook, prioritize OAuth over password storage
4. **Secure Storage Integration**: Platform-specific (Keychain on macOS, Credential Manager on Windows)

## Performance Considerations

### Indexes
- All foreign keys indexed for join performance
- Account and folder lookups optimized
- Email queries unchanged from single-account schema

### Connection Pooling
- Each account should have separate connection pool
- Pool size configurable per account based on usage
- Session tracking for health monitoring

### Cache Invalidation
- Switching accounts should not invalidate entire cache
- Cache queries filtered by account_id via folder relationship
- Memory cache keyed by `account_id:folder_name:uid`

## API Implications

### Configuration Changes
- Current: Single IMAP config in `Settings`
- Future: `Settings` becomes global config, account settings in database
- Account selection parameter added to all email operations

### Backward Compatibility
- Default account automatically selected if no account specified
- Legacy single-account code paths continue to work
- Gradual migration of APIs to account-aware versions

## UI/UX Considerations

### Account Switcher
- Quick-switch dropdown in UI header
- Shows account name and email address
- Indicates connection status (connected, error, disconnected)

### Account Management
- List all accounts with status
- Add/edit/remove accounts
- Set default account
- Test connection button

### First-Time Setup
- Wizard for first account
- Auto-detection based on email address
- Manual fallback for unsupported providers

## Testing Strategy

### Schema Validation
- ✅ Migration from v1 to v2 preserves data
- ✅ Unique constraints enforced (email, account name, single default)
- ✅ Foreign key cascades work correctly
- ✅ Indexes created successfully

### Multi-Account Scenarios
- Multiple accounts with different providers
- Switching between accounts
- Parallel syncing of multiple accounts
- Account deletion cleans up all related data

### Edge Cases
- Deleting default account (should prompt to set new default)
- Adding duplicate email addresses (should fail gracefully)
- Provider auto-detection with unknown domains (fallback to manual)
- OAuth token expiry handling

## Future Enhancements

1. **OAuth2 Implementation**
   - Google OAuth for Gmail
   - Microsoft OAuth for Outlook
   - Token refresh automation

2. **Account Sync Policies**
   - Per-account sync intervals
   - Selective folder syncing
   - Bandwidth/storage limits per account

3. **Unified Inbox**
   - Aggregate view across accounts
   - Smart filtering and search
   - Unified unread count

4. **Account Templates**
   - Save common configurations
   - Share configurations (export/import)
   - Organization-wide account provisioning

## Implementation Roadmap

- [x] Task 31.1: Database schema design and migration file
- [ ] Task 31.2: Auto-configuration for common providers
- [ ] Task 31.3: Manual configuration fallback
- [ ] Task 31.4: Account management UI
- [ ] Task 31.5: Account switching and session management
