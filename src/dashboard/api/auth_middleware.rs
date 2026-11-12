// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Authenticates every dashboard route (and the detailed health report and
//! metrics).
//!
//! A request is let through with either:
//! - the API key (`X-Api-Key` or `Authorization: Bearer`), as sent by MCP
//!   clients and scripts, or
//! - a dashboard session cookie (see `session.rs`), as sent by the browser,
//!   but only on a same-origin request (`Sec-Fetch-Site: same-origin`).
//!   SameSite=Strict does not separate ports, so this stops pages served
//!   from other localhost ports from riding on the cookie.
//!
//! A wrong key is refused even when a valid cookie is also present.
//!
//! Wrapped around the whole `/api/dashboard` scope, so a new route is
//! protected without any extra code. Two routes are exempt:
//! - the Microsoft OAuth callback: Microsoft redirects the browser there, so
//!   it cannot carry credentials; the one-time OAuth `state` protects it.
//! - `POST /session`: the login itself, which checks the key in its body.

use super::routes::{DASHBOARD_SCOPE, MICROSOFT_CALLBACK_PATH};
use super::session::{SESSION_COOKIE, SESSION_PATH};
use crate::api::api_key::{check_api_key, ApiKeyCheck};
use crate::dashboard::services::dashboard_sessions::DashboardSessions;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized};
use actix_web::http::Method;
use actix_web::{web, Error as ActixError};
use actix_web_lab::middleware::Next;
use log::warn;

const FETCH_SITE_HEADER: &str = "Sec-Fetch-Site";
const SAME_ORIGIN: &str = "same-origin";

fn dashboard_path_is(req: &ServiceRequest, method: Method, path: &str) -> bool {
    req.method() == method
        && req
            .path()
            .strip_prefix(DASHBOARD_SCOPE)
            .is_some_and(|rest| rest == path)
}

/// True only for the browser redirect from Microsoft's OAuth flow, or the login.
fn is_exempt(req: &ServiceRequest) -> bool {
    dashboard_path_is(req, Method::GET, MICROSOFT_CALLBACK_PATH)
        || dashboard_path_is(req, Method::POST, SESSION_PATH)
}

/// True when the request carries a live session cookie on a same-origin request.
fn has_valid_session(req: &ServiceRequest) -> bool {
    let Some(cookie) = req.cookie(SESSION_COOKIE) else {
        return false;
    };
    let same_origin = req
        .headers()
        .get(FETCH_SITE_HEADER)
        .and_then(|value| value.to_str().ok())
        == Some(SAME_ORIGIN);
    if !same_origin {
        warn!(
            "Session cookie refused: not a same-origin request: {}",
            req.path()
        );
        return false;
    }
    req.app_data::<web::Data<DashboardSessions>>()
        .is_some_and(|sessions| sessions.is_valid(cookie.value()))
}

/// Middleware: refuse any request without the API key or a valid session.
pub async fn require_api_key(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, ActixError> {
    if is_exempt(&req) {
        return next.call(req).await;
    }
    match check_api_key(req.headers()) {
        ApiKeyCheck::Valid => next.call(req).await,
        ApiKeyCheck::Missing if has_valid_session(&req) => next.call(req).await,
        ApiKeyCheck::Missing => {
            warn!("Dashboard request without credentials: {}", req.path());
            Err(ErrorUnauthorized("API key or session required"))
        }
        ApiKeyCheck::Invalid => {
            warn!("Dashboard request with invalid API key: {}", req.path());
            Err(ErrorUnauthorized("Invalid API key"))
        }
        ApiKeyCheck::NotConfigured => {
            warn!("Dashboard request refused: RUSTYMAIL_API_KEY not configured");
            Err(ErrorInternalServerError(
                "Server configuration error: API key not configured",
            ))
        }
    }
}
