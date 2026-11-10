// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Requires `RUSTYMAIL_API_KEY` on every dashboard route (and on the
//! detailed health report and metrics).
//!
//! Wrapped around the whole `/api/dashboard` scope, so a new route is
//! protected without any extra code. The one exception is the Microsoft
//! OAuth callback: Microsoft redirects the browser there, so it cannot carry
//! the key, and it is protected instead by the one-time `state` value that
//! must match a pending authorization.

use super::routes::{DASHBOARD_SCOPE, MICROSOFT_CALLBACK_PATH};
use crate::api::api_key::{check_api_key, ApiKeyCheck};
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized};
use actix_web::http::Method;
use actix_web::Error as ActixError;
use actix_web_lab::middleware::Next;
use log::warn;

/// True only for the browser redirect from Microsoft's OAuth flow.
fn is_oauth_callback(req: &ServiceRequest) -> bool {
    req.method() == Method::GET
        && req
            .path()
            .strip_prefix(DASHBOARD_SCOPE)
            .is_some_and(|rest| rest == MICROSOFT_CALLBACK_PATH)
}

/// Middleware: refuse any request without the right API key.
pub async fn require_api_key(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, ActixError> {
    if is_oauth_callback(&req) {
        return next.call(req).await;
    }
    match check_api_key(req.headers()) {
        ApiKeyCheck::Valid => next.call(req).await,
        ApiKeyCheck::Missing => {
            warn!("Dashboard request without API key: {}", req.path());
            Err(ErrorUnauthorized("API key required"))
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
