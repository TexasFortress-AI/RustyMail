# Task ID: 74

**Title:** Add force parameter to sync trigger API endpoint

**Status:** done

**Dependencies:** 73 ✓

**Priority:** medium

**Description:** Extend the sync trigger API endpoint to accept a force parameter that passes the --force flag to the sync binary

**Details:**

In `src/dashboard/api/handlers.rs`, modify `trigger_email_sync()`:

```rust
#[derive(Deserialize)]
struct SyncParams {
    account_id: i32,
    folder_id: Option<i32>,
    force: Option<bool>,  // New optional parameter
}

pub async fn trigger_email_sync(
    Query(params): Query<SyncParams>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Build command arguments
    let mut args = vec![
        "--account-id".to_string(),
        params.account_id.to_string(),
    ];
    
    if let Some(folder_id) = params.folder_id {
        args.push("--folder-id".to_string());
        args.push(folder_id.to_string());
    }
    
    // Add force flag if requested
    if params.force.unwrap_or(false) {
        args.push("--force".to_string());
    }
    
    // Spawn sync process
    let mut cmd = Command::new("./target/release/sync");
    cmd.args(&args);
    
    match cmd.spawn() {
        Ok(_) => Ok(Json(json!({
            "status": "sync_started",
            "account_id": params.account_id,
            "folder_id": params.folder_id,
            "force": params.force.unwrap_or(false)
        }))),
        Err(e) => Err(AppError::InternalServerError(format!("Failed to start sync: {}", e)))
    }
}
```

Update route registration if needed to ensure query parameters are parsed correctly.

**Test Strategy:**

1. Test normal sync trigger: `POST /api/sync/trigger?account_id=1&folder_id=1`
2. Test force sync trigger: `POST /api/sync/trigger?account_id=1&folder_id=1&force=true`
3. Verify sync binary is spawned with correct arguments using process monitoring
4. Test with force=false explicitly to ensure it doesn't add the flag
5. Test error handling when sync binary is not found or fails to start
6. Verify API response includes force parameter status
