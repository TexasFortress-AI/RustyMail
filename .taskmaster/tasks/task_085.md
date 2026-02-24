# Task ID: 85

**Title:** Run One-Time Database Migration to Remove Existing Duplicates

**Status:** pending

**Dependencies:** 84

**Priority:** medium

**Description:** Clean up existing duplicate emails in the database by keeping only the most recent version of each message_id

**Details:**

Create a migration script to deduplicate existing data:

```sql
-- Migration: deduplicate_emails.sql
-- Keep the most recent row for each (account_id, folder, message_id) combination

BEGIN TRANSACTION;

-- Create temporary table with deduplicated data
CREATE TEMPORARY TABLE emails_deduped AS
SELECT DISTINCT ON (account_id, folder, message_id) *
FROM emails
ORDER BY account_id, folder, message_id, updated_at DESC, id DESC;

-- Delete all existing emails
DELETE FROM emails;

-- Re-insert deduplicated data
INSERT INTO emails SELECT * FROM emails_deduped;

-- Add unique constraint to prevent future duplicates
ALTER TABLE emails ADD CONSTRAINT unique_message_per_folder 
    UNIQUE (account_id, folder, message_id);

DROP TABLE emails_deduped;

COMMIT;
```

Implement in Rust migration system:
```rust
// In migrations/deduplicate_emails.rs
pub async fn run_deduplication_migration(pool: &SqlitePool) -> Result<()> {
    let mut tx = pool.begin().await?;
    
    // Log duplicate counts before migration
    let duplicate_count = sqlx::query_scalar!(
        "SELECT COUNT(*) - COUNT(DISTINCT account_id || folder || message_id) FROM emails"
    )
    .fetch_one(&mut tx)
    .await?;
    
    info!("Found {} duplicate emails to remove", duplicate_count);
    
    // Run deduplication
    sqlx::query!(include_str!("deduplicate_emails.sql"))
        .execute(&mut tx)
        .await?;
    
    tx.commit().await?;
    info!("Deduplication complete");
    Ok(())
}
```

**Test Strategy:**

1. Test migration on a copy of production database
2. Verify row counts before and after migration
3. Test that unique constraint prevents new duplicates
4. Verify no data loss - all unique emails preserved
5. Test rollback capability if migration fails
