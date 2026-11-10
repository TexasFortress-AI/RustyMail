// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Every `/api/dashboard` route requires `RUSTYMAIL_API_KEY`.
//!
//! The app is built with no `DashboardState`, so a request that passes the
//! key check fails in its handler's extractors (500/400) and never touches
//! mail. Only a 401 proves the key check refused it.

use actix_web::http::{Method, StatusCode};
use actix_web::{test, App};
use serial_test::serial;

const TEST_KEY: &str = "dashboard-auth-test-key";
const WRONG_KEY: &str = "dashboard-auth-wrong-key";

/// Every route registered in `src/dashboard/api/routes.rs`, except the
/// OAuth callback, which is tested separately.
const DASHBOARD_ROUTES: &[(&str, &str)] = &[
    ("GET", "/stats"),
    ("GET", "/clients"),
    ("GET", "/config"),
    ("PUT", "/config/imap"),
    ("PUT", "/config/rest"),
    ("PUT", "/config/dashboard"),
    ("GET", "/config/validate"),
    ("POST", "/chatbot/query"),
    ("POST", "/chatbot/stream"),
    ("GET", "/mcp/tools"),
    ("POST", "/mcp/execute"),
    ("GET", "/ai/providers"),
    ("POST", "/ai/providers/set"),
    ("GET", "/ai/models"),
    ("POST", "/ai/models/set"),
    ("GET", "/ai/model-configs"),
    ("POST", "/ai/model-configs"),
    ("GET", "/ai/models-for-provider"),
    ("GET", "/ai/sampler-configs"),
    ("POST", "/ai/sampler-configs"),
    ("DELETE", "/ai/sampler-configs"),
    ("GET", "/ai/sampler-configs/list"),
    ("GET", "/ai/sampler-configs/defaults"),
    ("GET", "/ai/sampler-configs/presets"),
    ("POST", "/ai/sampler-configs/presets/import"),
    ("POST", "/sync/trigger"),
    ("POST", "/sync/flags"),
    ("GET", "/sync/status"),
    ("GET", "/folders"),
    ("GET", "/cached-folders"),
    ("GET", "/emails"),
    ("POST", "/emails/send"),
    ("POST", "/emails/delete"),
    ("GET", "/events"),
    ("POST", "/accounts/auto-config"),
    ("POST", "/accounts"),
    ("GET", "/accounts"),
    ("GET", "/accounts/default"),
    ("GET", "/accounts/a@example.test"),
    ("PUT", "/accounts/a@example.test"),
    ("DELETE", "/accounts/a@example.test"),
    ("POST", "/accounts/a@example.test/default"),
    ("GET", "/accounts/a@example.test/connection-status"),
    ("POST", "/accounts/a@example.test/validate"),
    ("GET", "/events/types"),
    ("GET", "/clients/c1/subscriptions"),
    ("PUT", "/clients/c1/subscriptions"),
    ("POST", "/clients/c1/subscribe"),
    ("POST", "/clients/c1/unsubscribe"),
    ("GET", "/attachments/list"),
    ("GET", "/attachments/m1/zip"),
    ("GET", "/attachments/m1/inline/c1"),
    ("GET", "/attachments/m1/file.pdf"),
    ("GET", "/jobs"),
    ("DELETE", "/jobs/finished"),
    ("POST", "/jobs/cancel"),
    ("POST", "/jobs/pause"),
    ("POST", "/jobs/resume"),
    ("POST", "/jobs/process-emails"),
    ("GET", "/jobs/j1"),
    ("DELETE", "/jobs/j1"),
    ("GET", "/oauth/status"),
    ("GET", "/oauth/microsoft/authorize"),
    // Not a route: the whole scope is wrapped, not just the listed routes.
    ("GET", "/no-such-route"),
];

enum Credential {
    None,
    Header(&'static str),
    Bearer(&'static str),
}

async fn status_for(method: &str, path: &str, credential: Credential) -> StatusCode {
    let app =
        test::init_service(App::new().configure(rustymail::dashboard::api::init_routes)).await;
    let mut req = test::TestRequest::default()
        .method(Method::from_bytes(method.as_bytes()).unwrap())
        .uri(path);
    req = match credential {
        Credential::None => req,
        Credential::Header(key) => req.insert_header(("X-API-Key", key)),
        Credential::Bearer(key) => req.insert_header(("Authorization", format!("Bearer {}", key))),
    };
    // Middleware errors become HTTP responses in the server; here they come
    // back as `Err`, so read the status the server would send.
    match test::try_call_service(&app, req.to_request()).await {
        Ok(response) => response.status(),
        Err(error) => error.as_response_error().status_code(),
    }
}

fn dashboard_path(route: &str) -> String {
    format!("/api/dashboard{}", route)
}

#[actix_web::test]
#[serial]
async fn every_dashboard_route_refuses_a_missing_key() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    for (method, route) in DASHBOARD_ROUTES {
        let status = status_for(method, &dashboard_path(route), Credential::None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{} {}", method, route);
    }
}

#[actix_web::test]
#[serial]
async fn every_dashboard_route_refuses_a_wrong_key() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    for (method, route) in DASHBOARD_ROUTES {
        let path = dashboard_path(route);
        let header = status_for(method, &path, Credential::Header(WRONG_KEY)).await;
        assert_eq!(header, StatusCode::UNAUTHORIZED, "{} {}", method, route);
        let bearer = status_for(method, &path, Credential::Bearer(WRONG_KEY)).await;
        assert_eq!(
            bearer,
            StatusCode::UNAUTHORIZED,
            "{} {} (bearer)",
            method,
            route
        );
    }
}

#[actix_web::test]
#[serial]
async fn every_dashboard_route_accepts_the_right_key() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    for (method, route) in DASHBOARD_ROUTES {
        let path = dashboard_path(route);
        let header = status_for(method, &path, Credential::Header(TEST_KEY)).await;
        assert_ne!(header, StatusCode::UNAUTHORIZED, "{} {}", method, route);
        let bearer = status_for(method, &path, Credential::Bearer(TEST_KEY)).await;
        assert_ne!(
            bearer,
            StatusCode::UNAUTHORIZED,
            "{} {} (bearer)",
            method,
            route
        );
    }
}

#[actix_web::test]
#[serial]
async fn no_configured_key_refuses_every_request() {
    std::env::remove_var("RUSTYMAIL_API_KEY");
    let unset = status_for("GET", "/api/dashboard/stats", Credential::Header("")).await;
    std::env::set_var("RUSTYMAIL_API_KEY", "your-secure-api-key-here");
    let placeholder = status_for(
        "GET",
        "/api/dashboard/stats",
        Credential::Header("your-secure-api-key-here"),
    )
    .await;
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    assert_eq!(unset, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(placeholder, StatusCode::INTERNAL_SERVER_ERROR);
}

/// Microsoft redirects the browser here without a key; the one-time OAuth
/// `state` protects it instead. Only GET is exempt.
#[actix_web::test]
#[serial]
async fn only_the_oauth_callback_get_is_exempt() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let callback = "/api/dashboard/oauth/callback/microsoft";
    assert_ne!(
        status_for("GET", callback, Credential::None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status_for("POST", callback, Credential::None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status_for("GET", &format!("{}/extra", callback), Credential::None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
#[serial]
async fn health_report_and_metrics_need_the_key_but_probes_do_not() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    for path in ["/health/report", "/health/metrics"] {
        assert_eq!(
            status_for("GET", path, Credential::None).await,
            StatusCode::UNAUTHORIZED,
            "{}",
            path
        );
        assert_ne!(
            status_for("GET", path, Credential::Header(TEST_KEY)).await,
            StatusCode::UNAUTHORIZED,
            "{}",
            path
        );
    }
    for path in ["/healthz", "/readyz", "/health/live", "/health/ready"] {
        assert_ne!(
            status_for("GET", path, Credential::None).await,
            StatusCode::UNAUTHORIZED,
            "{}",
            path
        );
    }
}
