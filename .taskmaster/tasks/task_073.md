# Task ID: 73

**Title:** Add force re-sync CLI flag to sync binary

**Status:** done

**Dependencies:** None

**Priority:** medium

**Description:** Implement a --force flag in the sync binary that allows users to re-download all emails by resetting the sync checkpoint

**Details:**

1. Update CLI struct in `src/bin/sync.rs`:
```rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Account ID to sync
    #[arg(short, long)]
    account_id: i32,
    
    /// Folder ID to sync (optional, syncs all if not specified)
    #[arg(short, long)]
    folder_id: Option<i32>,
    
    /// Force full re-sync by resetting sync checkpoint
    #[arg(long, help = "Force re-download all emails by resetting sync checkpoint")]
    force: bool,
}
```

2. Pass force flag through the sync chain:
```rust
// In main()
if let Some(folder_id) = cli.folder_id {
    sync_folder(&pool, cli.account_id, folder_id, cli.force).await?;
} else {
    sync_all_folders(&pool, cli.account_id, cli.force).await?;
}

// Update function signatures
async fn sync_folder(pool: &PgPool, account_id: i32, folder_id: i32, force: bool) -> Result<()> {
    if force {
        // Reset last_uid_synced to 0 before starting sync
        sqlx::query!(
            "UPDATE sync_state SET last_uid_synced = 0 WHERE account_id = $1 AND folder_id = $2",
            account_id, folder_id
        ).execute(pool).await?;
    }
    // Continue with normal sync logic
}
```

3. Update IMAP search criteria when force=true:
```rust
let search_criteria = if force || last_uid_synced == 0 {
    "ALL".to_string()
} else {
    format!("UID {}:*", last_uid_synced + 1)
};
```

**Test Strategy:**

1. Build sync binary: `cargo build --bin sync`
2. Test normal sync: `./target/debug/sync --account-id 1 --folder-id 1`
3. Test force sync: `./target/debug/sync --account-id 1 --folder-id 1 --force`
4. Verify force sync resets last_uid_synced to 0 in database
5. Verify force sync re-downloads all emails even if already cached
6. Test --help flag shows force option documentation
