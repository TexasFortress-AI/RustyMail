// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! OAuth2 API endpoints for Microsoft 365 account linking.

use actix_web::{web, HttpResponse};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::dashboard::services::DashboardState;

/// Response for the authorize endpoint.
#[derive(Debug, Serialize)]
struct AuthorizeResponse {
    authorization_url: String,
    state: String,
}

/// Query parameters from the Microsoft OAuth callback.
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Response after successful token exchange.
#[derive(Debug, Serialize)]
struct CallbackResponse {
    success: bool,
    email: Option<String>,
    message: String,
}

/// GET /api/dashboard/oauth/microsoft/authorize
///
/// Returns the Microsoft OAuth2 authorization URL for the frontend to redirect to.
pub async fn microsoft_authorize(
    state: web::Data<DashboardState>,
) -> HttpResponse {
    let oauth_service = &state.oauth_service;

    if !oauth_service.is_microsoft_configured() {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Microsoft OAuth is not configured. Set MICROSOFT_CLIENT_ID, MICROSOFT_CLIENT_SECRET, and OAUTH_REDIRECT_BASE_URL."
        }));
    }

    match oauth_service.generate_microsoft_auth_url().await {
        Ok((auth_url, oauth_state)) => {
            info!("Generated Microsoft OAuth2 authorization URL");
            HttpResponse::Ok().json(AuthorizeResponse {
                authorization_url: auth_url,
                state: oauth_state,
            })
        }
        Err(e) => {
            error!("Failed to generate authorization URL: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to generate authorization URL: {}", e)
            }))
        }
    }
}

/// GET /api/dashboard/oauth/callback/microsoft
///
/// Handles the Microsoft OAuth2 callback. Exchanges the authorization code
/// for access + refresh tokens and creates/updates the email account.
pub async fn microsoft_callback(
    query: web::Query<OAuthCallbackQuery>,
    state: web::Data<DashboardState>,
) -> HttpResponse {
    // Check for error from Microsoft
    if let Some(error) = &query.error {
        let desc = query.error_description.as_deref().unwrap_or("Unknown error");
        error!("Microsoft OAuth callback error: {} - {}", error, desc);
        return HttpResponse::BadRequest().json(CallbackResponse {
            success: false,
            email: None,
            message: format!("Authorization denied: {}", desc),
        });
    }

    let code = match &query.code {
        Some(c) => c,
        None => {
            return HttpResponse::BadRequest().json(CallbackResponse {
                success: false,
                email: None,
                message: "Missing authorization code".to_string(),
            });
        }
    };

    let oauth_state = match &query.state {
        Some(s) => s,
        None => {
            return HttpResponse::BadRequest().json(CallbackResponse {
                success: false,
                email: None,
                message: "Missing state parameter".to_string(),
            });
        }
    };

    let oauth_service = &state.oauth_service;

    // Exchange authorization code for tokens
    let token_response = match oauth_service.exchange_code(oauth_state, code).await {
        Ok(tokens) => tokens,
        Err(e) => {
            error!("Token exchange failed: {}", e);
            return HttpResponse::InternalServerError().json(CallbackResponse {
                success: false,
                email: None,
                message: format!("Token exchange failed: {}", e),
            });
        }
    };

    // Store the tokens in the account service
    // The actual account creation/update with OAuth tokens will be handled
    // by the account store integration (task 49).
    // For now, return the success with token info.
    info!("Microsoft OAuth2 token exchange successful (expires_in={}s)", token_response.expires_in);

    HttpResponse::Ok().json(CallbackResponse {
        success: true,
        email: None, // Will be populated when account store integration is done
        message: "OAuth authorization successful. Tokens received.".to_string(),
    })
}

/// GET /api/dashboard/oauth/status
///
/// Returns which OAuth providers are configured.
pub async fn oauth_status(
    state: web::Data<DashboardState>,
) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "microsoft": state.oauth_service.is_microsoft_configured(),
    }))
}
