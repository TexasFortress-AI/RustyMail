// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use log::{info, error};
use crate::dashboard::services::{DashboardState, Account, AutoConfigResult};

#[derive(Debug, Deserialize)]
pub struct AutoConfigRequest {
    pub email_address: String,
    pub password: Option<String>, // Optional for now, required for actual connection validation
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    #[serde(alias = "account_name")]
    pub display_name: String,
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
    #[serde(default)]
    pub is_default: bool,
    pub validate_connection: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    #[serde(alias = "account_name")]
    pub display_name: Option<String>,
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
    _state: web::Data<DashboardState>,
    req: web::Json<AutoConfigRequest>,
) -> HttpResponse {
    use crate::dashboard::services::autodiscovery::AutodiscoveryService;

    info!("Auto-configuring for email: {}", req.email_address);

    // Create autodiscovery service
    let autodiscovery_service = match AutodiscoveryService::new() {
        Ok(svc) => svc,
        Err(e) => {
            error!("Failed to create autodiscovery service: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to initialize autodiscovery: {}", e)
            }));
        }
    };

    // Attempt autodiscovery
    match autodiscovery_service.discover(&req.email_address).await {
        Ok(config) => {
            info!("Autodiscovery successful for {}", req.email_address);

            // Convert EmailConfig to AutoConfigResult
            let result = AutoConfigResult {
                provider_found: true,
                provider_type: Some("Auto-discovered".to_string()),
                display_name: Some("Auto-discovered".to_string()),
                imap_host: Some(config.imap_host),
                imap_port: Some(config.imap_port as i64),
                imap_use_tls: Some(config.imap_use_tls),
                smtp_host: config.smtp_host,
                smtp_port: config.smtp_port.map(|p| p as i64),
                smtp_use_tls: config.smtp_use_tls,
                smtp_use_starttls: config.smtp_use_starttls,
                supports_oauth: false,
                oauth_provider: None,
            };

            HttpResponse::Ok().json(AutoConfigResponse {
                success: true,
                config: result,
            })
        }
        Err(e) => {
            error!("Autodiscovery failed for {}: {}", req.email_address, e);
            HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": format!("Could not autodiscover email settings: {}", e)
            }))
        }
    }
}

/// Create a new account
pub async fn create_account(
    state: web::Data<DashboardState>,
    req: web::Json<CreateAccountRequest>,
) -> HttpResponse {
    info!("Creating account: {}", req.display_name);

    let account_service = state.account_service.lock().await;

    // Build Account struct from request
    let new_account = Account {
        email_address: req.email_address.clone(),
        id: req.email_address.clone(), // Set id to match email_address
        display_name: req.display_name.clone(),
        provider_type: req.provider_type.clone(),
        imap_host: req.imap_host.clone(),
        imap_port: req.imap_port,
        imap_user: req.imap_user.clone(),
        imap_pass: req.imap_pass.clone(),
        imap_use_tls: req.imap_use_tls,
        smtp_host: req.smtp_host.clone(),
        smtp_port: req.smtp_port,
        smtp_user: req.smtp_user.clone(),
        smtp_pass: req.smtp_pass.clone(),
        smtp_use_tls: req.smtp_use_tls,
        smtp_use_starttls: req.smtp_use_starttls,
        is_active: true,
        is_default: req.is_default,
        connection_status: None, // Will be populated after validation
    };

    // Validate connection if requested
    if req.validate_connection.unwrap_or(true) {
        if let Err(e) = account_service.validate_connection(&new_account).await {
            error!("Connection validation failed for account {}: {}", req.display_name, e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Connection validation failed: {}", e)
            }));
        }
    }

    // Create the account
    match account_service.create_account(new_account.clone()).await {
        Ok(account_id) => {
            info!("Successfully created account {} with ID {}", req.display_name, account_id);

            // If this is marked as default, set it
            if req.is_default {
                if let Err(e) = account_service.set_default_account(&account_id).await {
                    error!("Failed to set default account {}: {}", account_id, e);
                }
            }

            HttpResponse::Ok().json(AccountResponse {
                success: true,
                message: "Account created successfully".to_string(),
                account: Some(new_account),
            })
        },
        Err(e) => {
            error!("Failed to create account {}: {}", req.display_name, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to create account: {}", e)
            }))
        }
    }
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
    if let Some(name) = &req.display_name {
        account.display_name = name.clone();
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
    // Only update password if provided and non-empty
    if let Some(pass) = &req.imap_pass {
        if !pass.is_empty() {
            account.imap_pass = pass.clone();
        }
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
    // Only update SMTP password if provided and non-empty
    if let Some(smtp_pass) = &req.smtp_pass {
        if !smtp_pass.is_empty() {
            account.smtp_pass = Some(smtp_pass.clone());
        }
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

/// Get connection status for an account
pub async fn get_connection_status(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> HttpResponse {
    let account_id = path.into_inner();
    info!("Getting connection status for account ID: {}", account_id);

    let account_service = state.account_service.lock().await;

    match account_service.get_connection_status(&account_id).await {
        Ok(status) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "status": status
            }))
        },
        Err(e) => {
            error!("Failed to get connection status for account {}: {}", account_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to get connection status: {}", e)
            }))
        }
    }
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
