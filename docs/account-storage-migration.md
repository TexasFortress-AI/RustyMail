# Account Storage Migration Guide

## Overview

As of Task #37, RustyMail has migrated from database-based account storage to file-based storage using a JSON configuration file. This change ensures that email account credentials are persisted separately from the email cache database, allowing the cache database to be safely deleted without losing account information.

## Architecture

### Before (Database-based)
- Account credentials stored in `accounts` table in SQLite database
- Database located at: `data/email_cache.db` (or `CACHE_DATABASE_URL`)
- **Risk**: Deleting database would lose all account configurations

### After (File-based)
- Account credentials stored in: `config/accounts.json` (or `ACCOUNTS_CONFIG_PATH`)
- Database only stores:
  - Provider templates (Gmail, Outlook, etc.)
  - Email cache (headers, bodies, folders)
- **Safe**: Database can be deleted without losing account information

## File Structure

### accounts.json Format
```json
{
  "version": "1.0",
  "default_account_id": "account_1234567890",
  "accounts": [
    {
      "id": "account_1234567890",
      "account_name": "Personal Gmail",
      "email_address": "user@gmail.com",
      "provider_type": "gmail",
      "imap": {
        "host": "imap.gmail.com",
        "port": 993,
        "username": "user@gmail.com",
        "password": "your-app-password",
        "use_tls": true
      },
      "smtp": {
        "host": "smtp.gmail.com",
        "port": 587,
        "username": "user@gmail.com",
        "password": "your-app-password",
        "use_tls": true,
        "use_starttls": true
      },
      "is_active": true,
      "created_at": "2025-10-02T16:00:00Z",
      "updated_at": "2025-10-02T16:00:00Z"
    }
  ]
}
```

### File Permissions
- Unix systems: File is created with mode `0600` (read/write for owner only)
- Ensures passwords are protected from other users

### Atomic Writes
- All writes use temporary file + atomic rename pattern
- Prevents data corruption if process is interrupted
- Format: `accounts.json.tmp` → `accounts.json`

## Automatic Migration

The system automatically migrates existing database accounts to file storage on first run:

### Migration Process
1. **Initialization**: `AccountService::initialize()` is called
2. **Check**: Looks for existing `accounts` table in database
3. **Verify**: Checks if `accounts.json` already has accounts
4. **Migrate**: If database has accounts but file doesn't:
   - Copies all accounts from database to file
   - Preserves default account designation
   - Maintains IMAP/SMTP configurations
5. **Log**: Records migration success/failure

### Migration Safety
- **Idempotent**: Safe to run multiple times
- **Non-destructive**: Does not delete database accounts
- **Graceful failure**: Continues with file-based storage even if migration fails

## Configuration

### Environment Variables

```bash
# Account configuration file path (default: config/accounts.json)
ACCOUNTS_CONFIG_PATH=config/accounts.json

# Cache database URL (still used for provider templates and email cache)
CACHE_DATABASE_URL=sqlite:data/email_cache.db
```

## Testing Database Deletion Safety

### Test Procedure

1. **Backup current state** (if desired):
   ```bash
   cp data/email_cache.db data/email_cache.db.backup
   cp config/accounts.json config/accounts.json.backup
   ```

2. **Delete the database**:
   ```bash
   rm data/email_cache.db
   ```

3. **Start the server**:
   ```bash
   cargo run --bin rustymail-server
   ```

4. **Verify**:
   - Server starts successfully
   - Database is recreated with migrations
   - Accounts are loaded from `config/accounts.json`
   - Email functionality works with existing accounts
   - No account credentials are lost

### Expected Behavior

When the database is deleted and server restarts:

1. ✅ Database file is recreated automatically
2. ✅ Migrations create necessary tables (provider_templates, folders, etc.)
3. ✅ Accounts are loaded from `config/accounts.json`
4. ✅ IMAP connections work with file-based account credentials
5. ✅ Email cache is empty (expected - was in deleted database)
6. ✅ New emails can be synced and cached
7. ✅ No error messages about missing accounts

### What Gets Lost (Expected)

- ✅ Email cache (headers, bodies, folder structure)
- ✅ Sync state
- ✅ Local folder mappings

### What Is Preserved

- ✅ Account credentials
- ✅ Account names and settings
- ✅ IMAP/SMTP configuration
- ✅ Default account designation
- ✅ Active/inactive status

## API Changes

### AccountService Method Signatures

Methods now use string-based account IDs instead of i64:

```rust
// Old (database-based)
pub async fn get_account(&self, account_id: i64) -> Result<Account, AccountError>
pub async fn delete_account(&self, account_id: i64) -> Result<(), AccountError>
pub async fn create_account(&self, account: Account) -> Result<i64, AccountError>

// New (file-based)
pub async fn get_account(&self, account_id: &str) -> Result<Account, AccountError>
pub async fn delete_account(&self, account_id: &str) -> Result<(), AccountError>
pub async fn create_account(&self, account: Account) -> Result<String, AccountError>
```

### Account ID Format

- Old: Sequential integers (1, 2, 3, ...)
- New: Timestamp-based strings ("account_1727890123456")

## Migration from Old System

If upgrading from an older version:

1. **Automatic**: Migration happens automatically on first startup
2. **Manual**: If migration fails, manually create `config/accounts.json`:
   - Copy from `config/accounts.json.example`
   - Fill in your account details
   - Set appropriate permissions: `chmod 600 config/accounts.json`

## Rollback (if needed)

To revert to database-based storage (not recommended):

1. Restore the database backup
2. Check out previous git commit before file-based storage
3. Rebuild and restart

## Security Considerations

1. **File Permissions**: `accounts.json` contains plaintext passwords
   - Protected by Unix permissions (0600)
   - Not readable by other users
   - Should be in `.gitignore`

2. **Backup Strategy**:
   - Include `config/accounts.json` in backups
   - Exclude from version control
   - Store encrypted backups offsite

3. **Password Security**:
   - Use app-specific passwords when possible
   - Rotate passwords regularly
   - Consider implementing encryption at rest (future enhancement)

## Troubleshooting

### Issue: "Account not found" after database deletion
- **Cause**: `accounts.json` doesn't exist or is empty
- **Solution**: Check `config/accounts.json` exists and contains your accounts
- **Recovery**: Restore from backup or manually recreate

### Issue: "Failed to initialize account service"
- **Cause**: Permission error on `accounts.json` or parent directory
- **Solution**: Check file permissions and directory ownership
- **Fix**: `chmod 600 config/accounts.json && chown $USER config/accounts.json`

### Issue: "Migration failed" warning on startup
- **Cause**: Database accounts table doesn't exist or is corrupted
- **Solution**: Create accounts manually in `accounts.json`
- **Note**: This is a warning, not an error - system continues with file storage

## Future Enhancements

Potential improvements for account storage:

1. **Encryption**: Encrypt passwords in `accounts.json`
2. **Key Management**: Use system keyring for password storage
3. **OAuth Tokens**: Store OAuth refresh tokens securely
4. **Multi-user**: Support for multiple system users
5. **Cloud Sync**: Sync account configuration across devices

## Related Documentation

- `config/accounts.json.example` - Example configuration file
- `src/dashboard/services/account_store.rs` - File-based storage implementation
- `src/dashboard/services/account.rs` - Account service with migration logic
- `migrations/002_multi_account_support.sql` - Database schema (deprecated for accounts)
