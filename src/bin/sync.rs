// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Standalone email sync binary.
//!
//! This binary runs email sync in a separate process that exits after each sync cycle.
//! When the process exits, the OS reclaims ALL memory, solving the memory growth issue
//! where allocators hold freed memory for reuse.
//!
//! Usage: rustymail-sync [--database-url <URL>]
//!
//! The main server spawns this binary periodically. SQLite is the communication channel.

use clap::Parser;
use log::{info, error, warn, debug};
use sqlx::{SqlitePool, Row};
use std::fs::File;
use std::io::Write as IoWrite;
use chrono::Utc;

// Use jemalloc for consistency with main server
#[cfg(all(not(target_env = "msvc"), not(feature = "system-alloc"), not(feature = "mimalloc-alloc")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
#[command(name = "rustymail-sync", about = "Standalone email sync process")]
struct Cli {
    #[arg(long, env = "CACHE_DATABASE_URL", default_value = "sqlite:data/email_cache.db")]
    database_url: String,
}

/// Account row from database
struct AccountRow {
    email_address: String,
    imap_host: String,
    imap_port: i64,
    imap_user: String,
    imap_pass: String,
    imap_use_tls: bool,
}

/// Check if a process with the given PID is still running
#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    // On Unix: kill(pid, 0) returns success if process exists
    // We use the raw libc call to avoid adding libc as a dependency
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn process_exists(_pid: u32) -> bool {
    // On non-Unix, assume process doesn't exist (lock will be acquired)
    false
}

/// Acquire a lock file with crash recovery.
/// Returns the lock file handle on success.
fn acquire_lock() -> Result<File, String> {
    let lock_path = "data/.sync.lock";

    // Check for existing lock
    if let Ok(contents) = std::fs::read_to_string(lock_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            // Check if process is still running
            if process_exists(pid) {
                return Err(format!("Another sync is already running (pid: {})", pid));
            }
            // Stale lock - process crashed, remove it
            info!("Removing stale lock from crashed process {}", pid);
            if let Err(e) = std::fs::remove_file(lock_path) {
                return Err(format!("Failed to remove stale lock: {}", e));
            }
        }
    }

    // Create new lock with our PID
    let mut file = File::create(lock_path)
        .map_err(|e| format!("Failed to create lock file: {}", e))?;
    write!(file, "{}", std::process::id())
        .map_err(|e| format!("Failed to write PID to lock file: {}", e))?;

    Ok(file)
}

/// Remove the lock file on exit
fn release_lock() {
    let _ = std::fs::remove_file("data/.sync.lock");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cli = Cli::parse();

    info!("Starting email sync process (pid: {})", std::process::id());

    // Acquire lock with crash recovery
    let _lock = match acquire_lock() {
        Ok(f) => f,
        Err(e) => {
            info!("{}", e);
            return Ok(());
        }
    };

    // Ensure lock file is removed on exit
    // Using a simple struct with Drop instead of scopeguard dependency
    struct LockGuard;
    impl Drop for LockGuard {
        fn drop(&mut self) {
            release_lock();
        }
    }
    let _cleanup = LockGuard;

    // Connect to database
    let pool = SqlitePool::connect(&cli.database_url).await?;
    info!("Connected to database: {}", cli.database_url);

    // Read active accounts
    let rows = sqlx::query(
        r#"
        SELECT email_address, imap_host, imap_port, imap_user, imap_pass, imap_use_tls
        FROM accounts WHERE is_active = 1
        "#
    )
    .fetch_all(&pool)
    .await?;

    if rows.is_empty() {
        info!("No active accounts found, exiting");
        return Ok(());
    }

    let accounts: Vec<AccountRow> = rows.iter().map(|row| {
        AccountRow {
            email_address: row.get("email_address"),
            imap_host: row.get("imap_host"),
            imap_port: row.get("imap_port"),
            imap_user: row.get("imap_user"),
            imap_pass: row.get("imap_pass"),
            imap_use_tls: row.get("imap_use_tls"),
        }
    }).collect();

    info!("Found {} active accounts to sync", accounts.len());

    // Sync each account
    for account in accounts {
        if let Err(e) = sync_account(&pool, &account).await {
            error!("Failed to sync {}: {}", account.email_address, e);
        }
    }

    info!("Sync complete, exiting");
    Ok(())
}

/// Sync all folders for a single account
async fn sync_account(pool: &SqlitePool, account: &AccountRow) -> Result<(), Box<dyn std::error::Error>> {
    info!("Syncing account: {}", account.email_address);

    // Create IMAP session
    let client = rustymail::imap::client::ImapClient::<rustymail::imap::session::AsyncImapSessionWrapper>::connect(
        &account.imap_host,
        account.imap_port as u16,
        &account.imap_user,
        &account.imap_pass,
    ).await?;

    info!("Connected to IMAP server {} for {}", account.imap_host, account.email_address);

    // List folders
    let folders = client.list_folders().await?;
    info!("Found {} folders for {}", folders.len(), account.email_address);

    // Sync each folder
    for folder in &folders {
        if let Err(e) = sync_folder(pool, &client, &account.email_address, folder).await {
            warn!("Failed to sync folder {} for {}: {}", folder, account.email_address, e);
            // Continue with other folders
        }
    }

    // IMPORTANT: Logout to release BytePool buffers
    if let Err(e) = client.logout().await {
        warn!("Failed to logout IMAP session: {}", e);
    }

    info!("Finished syncing account: {}", account.email_address);
    Ok(())
}

/// Sync a single folder for an account
async fn sync_folder(
    pool: &SqlitePool,
    client: &rustymail::imap::client::ImapClient<rustymail::imap::session::AsyncImapSessionWrapper>,
    account_email: &str,
    folder_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Syncing folder: {} for {}", folder_name, account_email);

    // Select folder
    client.select_folder(folder_name).await?;

    // Get last synced UID
    let last_uid_synced = get_last_uid(pool, folder_name, account_email).await?;

    // Search for new emails
    let search_criteria = if last_uid_synced > 0 {
        format!("UID {}:*", last_uid_synced + 1)
    } else {
        "ALL".to_string()
    };

    let uids = client.search_emails(&search_criteria).await?;

    if uids.is_empty() {
        debug!("No new emails in folder {}", folder_name);
        return Ok(());
    }

    info!("Syncing {} emails in folder {} for {}", uids.len(), folder_name, account_email);

    // Process in batches of 100
    const BATCH_SIZE: usize = 100;
    let mut max_uid = last_uid_synced;

    for chunk in uids.chunks(BATCH_SIZE) {
        let emails = client.fetch_emails(chunk).await?;

        for email in &emails {
            if let Err(e) = cache_email(pool, folder_name, email, account_email).await {
                error!("Failed to cache email {}: {}", email.uid, e);
            } else {
                if email.uid > max_uid {
                    max_uid = email.uid;
                }
            }
        }

        // Explicitly drop to free memory
        drop(emails);
    }

    // Update sync state
    update_sync_state(pool, folder_name, max_uid, account_email).await?;

    info!("Synced {} emails in folder {}", uids.len(), folder_name);
    Ok(())
}


/// Get the last synced UID for a folder
async fn get_last_uid(pool: &SqlitePool, folder_name: &str, account_id: &str) -> Result<u32, sqlx::Error> {
    // First get folder_id
    let folder_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM folders WHERE name = ? AND account_id = ?"
    )
    .bind(folder_name)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    let folder_id = match folder_id {
        Some(id) => id,
        None => return Ok(0), // Folder doesn't exist yet, start from 0
    };

    let result: Option<i64> = sqlx::query_scalar(
        "SELECT last_uid_synced FROM sync_state WHERE folder_id = ?"
    )
    .bind(folder_id)
    .fetch_optional(pool)
    .await?;

    Ok(result.unwrap_or(0) as u32)
}

/// Update sync state with new last UID
async fn update_sync_state(pool: &SqlitePool, folder_name: &str, last_uid: u32, account_id: &str) -> Result<(), sqlx::Error> {
    // Get folder_id first
    let folder_id = get_or_create_folder_id(pool, folder_name, account_id).await?;

    sqlx::query(
        r#"
        INSERT INTO sync_state (folder_id, last_uid_synced, sync_status, updated_at)
        VALUES (?, ?, 'Idle', datetime('now'))
        ON CONFLICT(folder_id) DO UPDATE SET
            last_uid_synced = excluded.last_uid_synced,
            sync_status = 'Idle',
            updated_at = datetime('now')
        "#
    )
    .bind(folder_id)
    .bind(last_uid as i64)
    .execute(pool)
    .await?;
    Ok(())
}

/// Cache an email to the database
/// This matches the schema used by CacheService in cache.rs
async fn cache_email(
    pool: &SqlitePool,
    folder_name: &str,
    email: &rustymail::imap::Email,
    account_id: &str,
) -> Result<(), sqlx::Error> {
    // Get or create folder_id first
    let folder_id = get_or_create_folder_id(pool, folder_name, account_id).await?;

    // Extract data from envelope (matches cache.rs logic)
    let (message_id, subject, from_str, from_name_str, to_vec, cc_vec, parsed_date) =
        if let Some(envelope) = &email.envelope {
            let from_addr = envelope.from.first();
            let from_address = from_addr.map(|a| format!("{}@{}",
                a.mailbox.as_deref().unwrap_or(""),
                a.host.as_deref().unwrap_or(""))).unwrap_or_default();
            let from_name = from_addr.and_then(|a| a.name.clone());

            let to_addresses: Vec<String> = envelope.to.iter()
                .map(|a| format!("{}@{}", a.mailbox.as_deref().unwrap_or(""), a.host.as_deref().unwrap_or("")))
                .collect();
            let cc_addresses: Vec<String> = envelope.cc.iter()
                .map(|a| format!("{}@{}", a.mailbox.as_deref().unwrap_or(""), a.host.as_deref().unwrap_or("")))
                .collect();

            // Decode MIME-encoded subject if present
            let decoded_subject = envelope.subject.as_ref()
                .map(|s| rustymail::utils::decode_mime_header(s));

            // Parse envelope date string to DateTime<Utc>
            let date = envelope.date.as_ref().and_then(|date_str| {
                chrono::DateTime::parse_from_rfc2822(date_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
                    .or_else(|| {
                        chrono::DateTime::parse_from_rfc3339(date_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .ok()
                    })
            });

            (envelope.message_id.clone(), decoded_subject,
             Some(from_address), from_name, to_addresses, cc_addresses, date)
        } else {
            (None, None, None, None, Vec::new(), Vec::new(), None)
        };

    // Serialize arrays to JSON
    let to_addresses_json = serde_json::to_string(&to_vec).unwrap_or_else(|_| "[]".to_string());
    let cc_addresses_json = serde_json::to_string(&cc_vec).unwrap_or_else(|_| "[]".to_string());
    let flags_json = serde_json::to_string(&email.flags).unwrap_or_else(|_| "[]".to_string());

    let has_attachments = !email.attachments.is_empty();

    // Insert or update email in database (matches cache.rs schema)
    sqlx::query(
        r#"
        INSERT INTO emails (
            folder_id, uid, message_id, subject, from_address, from_name,
            to_addresses, cc_addresses, date, internal_date, size, flags,
            headers, body_text, body_html, has_attachments
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(folder_id, uid) DO UPDATE SET
            message_id = excluded.message_id,
            subject = excluded.subject,
            from_address = excluded.from_address,
            from_name = excluded.from_name,
            to_addresses = excluded.to_addresses,
            cc_addresses = excluded.cc_addresses,
            date = excluded.date,
            internal_date = excluded.internal_date,
            size = excluded.size,
            flags = excluded.flags,
            headers = excluded.headers,
            body_text = excluded.body_text,
            body_html = excluded.body_html,
            has_attachments = excluded.has_attachments,
            updated_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(folder_id)
    .bind(email.uid as i64)
    .bind(&message_id)
    .bind(&subject)
    .bind(&from_str)
    .bind(&from_name_str)
    .bind(&to_addresses_json)
    .bind(&cc_addresses_json)
    .bind(parsed_date)
    .bind(email.internal_date)
    .bind(email.body.as_ref().map(|b| b.len() as i64))
    .bind(&flags_json)
    .bind("{}")  // headers placeholder
    .bind(&email.text_body)
    .bind(&email.html_body)
    .bind(has_attachments)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get or create a folder_id for the given folder_name and account_id
async fn get_or_create_folder_id(pool: &SqlitePool, folder_name: &str, account_id: &str) -> Result<i64, sqlx::Error> {
    // First try to get existing folder
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM folders WHERE name = ? AND account_id = ?"
    )
    .bind(folder_name)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create the folder
    sqlx::query(
        "INSERT INTO folders (name, account_id, created_at) VALUES (?, ?, datetime('now'))"
    )
    .bind(folder_name)
    .bind(account_id)
    .execute(pool)
    .await?;

    // Get the new ID
    let id: i64 = sqlx::query_scalar(
        "SELECT id FROM folders WHERE name = ? AND account_id = ?"
    )
    .bind(folder_name)
    .bind(account_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}
