// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Dashboard login and logout.
//!
//! `POST /api/dashboard/session` takes the API key once and sets a session
//! cookie (HttpOnly, SameSite=Strict, scoped to the dashboard API). The
//! cookie carries a random token, never the key. `Secure` is not set
//! because the dashboard is served over plain HTTP on loopback.

use super::routes::DASHBOARD_SCOPE;
use crate::api::api_key::{configured_api_key, keys_match};
use crate::dashboard::services::dashboard_sessions::DashboardSessions;
use actix_web::cookie::{time, Cookie, SameSite};
use actix_web::{web, HttpRequest, HttpResponse};
use log::{info, warn};
use serde::Deserialize;

/// Name of the cookie holding the session token.
pub const SESSION_COOKIE: &str = "rustymail_session";
/// Path of the login/status/logout route, relative to the dashboard scope.
pub const SESSION_PATH: &str = "/session";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub api_key: String,
}

fn session_cookie(value: String, max_age: time::Duration) -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, value)
        .path(DASHBOARD_SCOPE)
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(max_age)
        .finish()
}

/// POST /api/dashboard/session: exchange the API key for a session cookie.
pub async fn login(
    sessions: Option<web::Data<DashboardSessions>>,
    body: web::Json<LoginRequest>,
) -> HttpResponse {
    let (Some(configured), Some(sessions)) = (configured_api_key(), sessions) else {
        warn!("Dashboard login refused: API key or session store not configured");
        return HttpResponse::InternalServerError().body("Server configuration error");
    };
    if !keys_match(&body.api_key, &configured) {
        warn!("Dashboard login refused: wrong API key");
        return HttpResponse::Unauthorized().body("Invalid API key");
    }
    let max_age = time::Duration::seconds(sessions.ttl().as_secs() as i64);
    info!("Dashboard login: session started");
    HttpResponse::NoContent()
        .cookie(session_cookie(sessions.create(), max_age))
        .finish()
}

/// GET /api/dashboard/session: 204 when the request is authenticated (the
/// middleware has already refused it otherwise).
pub async fn status() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

/// DELETE /api/dashboard/session: end the session and clear the cookie.
pub async fn logout(
    req: HttpRequest,
    sessions: Option<web::Data<DashboardSessions>>,
) -> HttpResponse {
    if let (Some(cookie), Some(sessions)) = (req.cookie(SESSION_COOKIE), sessions) {
        sessions.revoke(cookie.value());
    }
    info!("Dashboard logout");
    HttpResponse::NoContent()
        .cookie(session_cookie(String::new(), time::Duration::ZERO))
        .finish()
}
