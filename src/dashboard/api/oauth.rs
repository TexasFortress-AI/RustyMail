// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! OAuth2 API endpoints for Microsoft 365 account linking.

use actix_web::{web, HttpResponse};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use log::{error, info};
use serde::{Deserialize, Serialize};

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

    info!("Microsoft OAuth2 token exchange successful (expires_in={}s)", token_response.expires_in);

    // Extract email from the JWT access token's preferred_username claim
    let email = match extract_email_from_jwt(&token_response.access_token) {
        Some(e) => e,
        None => {
            error!("Could not extract email from access token JWT");
            return HttpResponse::InternalServerError().json(CallbackResponse {
                success: false,
                email: None,
                message: "Token received but could not identify account email".to_string(),
            });
        }
    };

    // Compute expiry as Unix timestamp
    let expires_at = chrono::Utc::now().timestamp() + token_response.expires_in as i64;

    // Persist tokens to the matching account
    let account_service = state.account_service.lock().await;
    if let Err(e) = account_service.update_oauth_tokens(
        &email,
        &token_response.access_token,
        token_response.refresh_token.as_deref(),
        expires_at,
    ).await {
        error!("Failed to persist OAuth tokens for {}: {}", email, e);
        return HttpResponse::InternalServerError().json(CallbackResponse {
            success: false,
            email: Some(email),
            message: format!("Token exchange succeeded but failed to save: {}", e),
        });
    }

    info!("OAuth tokens persisted for account: {}", email);

    HttpResponse::Ok().json(CallbackResponse {
        success: true,
        email: Some(email),
        message: "OAuth authorization successful. Account linked.".to_string(),
    })
}

/// Extract the `preferred_username` (email) from a Microsoft JWT access token.
///
/// Microsoft access tokens are JWTs with 3 base64url-encoded segments.
/// We decode the payload (segment 1) and extract the `preferred_username` field.
/// No signature verification needed — we just received this from Microsoft's token endpoint.
fn extract_email_from_jwt(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // JWT uses base64url encoding (no padding). The base64 crate's STANDARD
    // engine expects standard base64 with padding, so convert URL-safe chars.
    let payload_b64 = parts[1]
        .replace('-', "+")
        .replace('_', "/");

    // Add padding if needed
    let padded = match payload_b64.len() % 4 {
        2 => format!("{}==", payload_b64),
        3 => format!("{}=", payload_b64),
        _ => payload_b64,
    };

    let decoded = BASE64.decode(&padded).ok()?;
    let payload: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    // Microsoft tokens use "preferred_username" for the user's email, or fall back to "upn"
    payload.get("preferred_username")
        .or_else(|| payload.get("upn"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    /// Build a fake JWT with the given JSON payload (no real signature).
    fn fake_jwt(payload_json: &str) -> String {
        let header = BASE64.encode(b"{\"alg\":\"none\"}");
        let payload = BASE64.encode(payload_json.as_bytes())
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string();
        format!("{}.{}.sig", header, payload)
    }

    #[test]
    fn test_extract_email_preferred_username() {
        let jwt = fake_jwt(r#"{"preferred_username":"user@outlook.com","sub":"abc"}"#);
        assert_eq!(extract_email_from_jwt(&jwt), Some("user@outlook.com".to_string()));
    }

    #[test]
    fn test_extract_email_upn_fallback() {
        let jwt = fake_jwt(r#"{"upn":"admin@contoso.com","sub":"abc"}"#);
        assert_eq!(extract_email_from_jwt(&jwt), Some("admin@contoso.com".to_string()));
    }

    #[test]
    fn test_extract_email_no_email_field() {
        let jwt = fake_jwt(r#"{"sub":"abc","name":"Test User"}"#);
        assert_eq!(extract_email_from_jwt(&jwt), None);
    }

    #[test]
    fn test_extract_email_invalid_jwt() {
        assert_eq!(extract_email_from_jwt("not-a-jwt"), None);
        assert_eq!(extract_email_from_jwt(""), None);
    }
}
