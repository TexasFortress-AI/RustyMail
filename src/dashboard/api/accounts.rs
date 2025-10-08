use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use log::{info, error};
use crate::dashboard::services::{DashboardState, Account, AutoConfigResult};

#[derive(Debug, Deserialize)]
pub struct AutoConfigRequest {
    pub email_address: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub account_name: String,
    pub email_address: String,
    pub provider_type: Option<String>,
    pub imap_host: String,
    pub imap_port: i64,
    pub imap_user: String,
    pub imap_pass: String,
    pub imap_use_tls: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_use_tls: Option<bool>,
    pub smtp_use_starttls: Option<bool>,
    pub is_default: bool,
    pub validate_connection: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub account_name: Option<String>,
    pub email_address: Option<String>,
    pub provider_type: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i64>,
    pub imap_user: Option<String>,
    pub imap_pass: Option<String>,
    pub imap_use_tls: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_use_tls: Option<bool>,
    pub smtp_use_starttls: Option<bool>,
    pub is_active: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub success: bool,
    pub message: String,
    pub account: Option<Account>,
}

#[derive(Debug, Serialize)]
pub struct AccountListResponse {
    pub success: bool,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Serialize)]
pub struct AutoConfigResponse {
    pub success: bool,
    pub config: AutoConfigResult,
}

/// Auto-configure email settings based on email address
pub async fn auto_configure(
    state: web::Data<DashboardState>,
    req: web::Json<AutoConfigRequest>,
) -> HttpResponse {
    info!("Auto-configuring for email: {}", req.email_address);

    // Note: AccountService needs to be initialized and added to DashboardState
    // For now, return a placeholder response
    HttpResponse::NotImplemented().json(serde_json::json!({
        "success": false,
        "error": "AccountService not yet integrated into DashboardState"
    }))
}

/// Create a new account
pub async fn create_account(
    state: web::Data<DashboardState>,
    req: web::Json<CreateAccountRequest>,
) -> HttpResponse {
    info!("Creating account: {}", req.account_name);

    // Placeholder - AccountService needs to be added to DashboardState
    HttpResponse::NotImplemented().json(serde_json::json!({
        "success": false,
        "error": "AccountService not yet integrated into DashboardState"
    }))
}

/// Get account by ID
pub async fn get_account(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Getting account ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    match account_service.get_account(&account_id).await {
        Ok(account) => {
            HttpResponse::Ok().json(AccountResponse {
                success: true,
                message: "Account retrieved successfully".to_string(),
                account: Some(account),
            })
        },
        Err(e) => {
            error!("Failed to get account {}: {}", account_id, e);
            HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": format!("Account not found: {}", e)
            }))
        }
    }
}

/// List all accounts
pub async fn list_accounts(
    state: web::Data<DashboardState>,
) -> HttpResponse {
    info!("Listing all accounts");

    let account_service = state.account_service.lock().await;

    match account_service.list_accounts().await {
        Ok(accounts) => {
            HttpResponse::Ok().json(AccountListResponse {
                success: true,
                accounts,
            })
        },
        Err(e) => {
            error!("Failed to list accounts: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to list accounts: {}", e)
            }))
        }
    }
}

/// Update account
pub async fn update_account(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
    req: web::Json<UpdateAccountRequest>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Updating account ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    // Get existing account
    let mut account = match account_service.get_account(&account_id).await {
        Ok(acc) => acc,
        Err(e) => {
            error!("Failed to get account {}: {}", account_id, e);
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": format!("Account not found: {}", e)
            }));
        }
    };

    // Apply updates from request (only non-None fields)
    if let Some(name) = &req.account_name {
        account.account_name = name.clone();
    }
    if let Some(email) = &req.email_address {
        account.email_address = email.clone();
    }
    if let Some(provider) = &req.provider_type {
        account.provider_type = Some(provider.clone());
    }
    if let Some(host) = &req.imap_host {
        account.imap_host = host.clone();
    }
    if let Some(port) = req.imap_port {
        account.imap_port = port;
    }
    if let Some(user) = &req.imap_user {
        account.imap_user = user.clone();
    }
    if let Some(pass) = &req.imap_pass {
        account.imap_pass = pass.clone();
    }
    if let Some(use_tls) = req.imap_use_tls {
        account.imap_use_tls = use_tls;
    }
    if let Some(smtp_host) = &req.smtp_host {
        account.smtp_host = Some(smtp_host.clone());
    }
    if let Some(smtp_port) = req.smtp_port {
        account.smtp_port = Some(smtp_port);
    }
    if let Some(smtp_user) = &req.smtp_user {
        account.smtp_user = Some(smtp_user.clone());
    }
    if let Some(smtp_pass) = &req.smtp_pass {
        account.smtp_pass = Some(smtp_pass.clone());
    }
    if let Some(smtp_tls) = req.smtp_use_tls {
        account.smtp_use_tls = Some(smtp_tls);
    }
    if let Some(smtp_starttls) = req.smtp_use_starttls {
        account.smtp_use_starttls = Some(smtp_starttls);
    }
    if let Some(is_active) = req.is_active {
        account.is_active = is_active;
    }

    // Handle is_default separately - need to call set_default_account
    let set_as_default = req.is_default.unwrap_or(false);

    // Update the account
    match account_service.update_account(&account_id, account.clone()).await {
        Ok(()) => {
            // If setting as default, do that too
            if set_as_default {
                if let Err(e) = account_service.set_default_account(&account_id).await {
                    error!("Failed to set default account {}: {}", account_id, e);
                }
            }

            HttpResponse::Ok().json(AccountResponse {
                success: true,
                message: "Account updated successfully".to_string(),
                account: Some(account),
            })
        },
        Err(e) => {
            error!("Failed to update account {}: {}", account_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to update account: {}", e)
            }))
        }
    }
}

/// Delete account
pub async fn delete_account(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Deleting account ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    match account_service.delete_account(&account_id).await {
        Ok(()) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Account deleted successfully"
            }))
        },
        Err(e) => {
            error!("Failed to delete account {}: {}", account_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to delete account: {}", e)
            }))
        }
    }
}

/// Set default account
pub async fn set_default_account(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Setting default account to ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    match account_service.set_default_account(&account_id).await {
        Ok(()) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Default account set successfully"
            }))
        },
        Err(e) => {
            error!("Failed to set default account {}: {}", account_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to set default account: {}", e)
            }))
        }
    }
}

/// Get default account
pub async fn get_default_account(
    state: web::Data<DashboardState>,
) -> HttpResponse {
    info!("Getting default account");

    HttpResponse::NotImplemented().json(serde_json::json!({
        "success": false,
        "error": "AccountService not yet integrated into DashboardState"
    }))
}

/// Validate account connection
pub async fn validate_connection(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Validating connection for account ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    // First get the account
    let account = match account_service.get_account(&account_id).await {
        Ok(acc) => acc,
        Err(e) => {
            error!("Failed to get account {}: {}", account_id, e);
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": format!("Account not found: {}", e)
            }));
        }
    };

    // Then validate the connection
    match account_service.validate_connection(&account).await {
        Ok(()) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Connection validated successfully"
            }))
        },
        Err(e) => {
            error!("Connection validation failed for account {}: {}", account_id, e);
            HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Connection validation failed: {}", e)
            }))
        }
    }
}
