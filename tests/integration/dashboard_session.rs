// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Browser login: the key is exchanged once for an HttpOnly session cookie,
//! which then authenticates same-origin requests only.

use actix_web::http::{header, StatusCode};
use actix_web::{test, web, App};
use rustymail::dashboard::services::dashboard_sessions::DashboardSessions;
use serial_test::serial;
use std::time::Duration;

const TEST_KEY: &str = "dashboard-session-test-key";
const SESSION: &str = "/api/dashboard/session";
const TTL: Duration = Duration::from_secs(3600);

fn sessions(ttl: Duration) -> web::Data<DashboardSessions> {
    web::Data::new(DashboardSessions::new(ttl))
}

macro_rules! app {
    ($sessions:expr) => {
        test::init_service(
            App::new()
                .app_data($sessions.clone())
                .configure(rustymail::dashboard::api::init_routes),
        )
        .await
    };
}

async fn status<S, B>(app: &S, req: test::TestRequest) -> StatusCode
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<B>,
        Error = actix_web::Error,
    >,
{
    match test::try_call_service(app, req.to_request()).await {
        Ok(response) => response.status(),
        Err(error) => error.as_response_error().status_code(),
    }
}

/// Logs in and returns the raw Set-Cookie header.
async fn login<S, B>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<B>,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri(SESSION)
        .set_json(serde_json::json!({ "api_key": TEST_KEY }))
        .to_request();
    let response = test::call_service(app, req).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    response
        .headers()
        .get(header::SET_COOKIE)
        .expect("login sets a cookie")
        .to_str()
        .unwrap()
        .to_string()
}

/// `name=value` from a Set-Cookie header, for sending back.
fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

fn browser_get(path: &str, cookie: &str, fetch_site: Option<&str>) -> test::TestRequest {
    let mut req = test::TestRequest::get()
        .uri(path)
        .insert_header((header::COOKIE, cookie.to_string()));
    if let Some(site) = fetch_site {
        req = req.insert_header(("Sec-Fetch-Site", site));
    }
    req
}

#[actix_web::test]
#[serial]
async fn wrong_key_does_not_log_in() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let app = app!(store);
    let req = test::TestRequest::post()
        .uri(SESSION)
        .set_json(serde_json::json!({ "api_key": "wrong" }));
    assert_eq!(status(&app, req).await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
#[serial]
async fn login_sets_an_httponly_strict_cookie_that_is_not_the_key() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let app = app!(store);
    let set_cookie = login(&app).await;
    assert!(
        set_cookie.starts_with("rustymail_session="),
        "{}",
        set_cookie
    );
    assert!(set_cookie.contains("HttpOnly"), "{}", set_cookie);
    assert!(set_cookie.contains("SameSite=Strict"), "{}", set_cookie);
    assert!(set_cookie.contains("Path=/api/dashboard"), "{}", set_cookie);
    assert!(set_cookie.contains("Max-Age=3600"), "{}", set_cookie);
    let token = cookie_pair(&set_cookie)["rustymail_session=".len()..].to_string();
    assert_eq!(token.len(), 64);
    assert!(!set_cookie.contains(TEST_KEY));
}

#[actix_web::test]
#[serial]
async fn session_cookie_authenticates_same_origin_requests_only() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let app = app!(store);
    let cookie = cookie_pair(&login(&app).await);
    assert_eq!(
        status(&app, browser_get(SESSION, &cookie, Some("same-origin"))).await,
        StatusCode::NO_CONTENT
    );
    assert_ne!(
        status(
            &app,
            browser_get("/api/dashboard/stats", &cookie, Some("same-origin"))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    // Other localhost ports are same-site, not same-origin.
    for site in [None, Some("same-site"), Some("cross-site"), Some("none")] {
        assert_eq!(
            status(&app, browser_get(SESSION, &cookie, site)).await,
            StatusCode::UNAUTHORIZED,
            "{:?}",
            site
        );
    }
}

#[actix_web::test]
#[serial]
async fn forged_cookie_and_wrong_key_beside_a_valid_cookie_are_refused() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let app = app!(store);
    let forged = format!("rustymail_session={}", "ab".repeat(32));
    assert_eq!(
        status(&app, browser_get(SESSION, &forged, Some("same-origin"))).await,
        StatusCode::UNAUTHORIZED
    );
    let cookie = cookie_pair(&login(&app).await);
    let req =
        browser_get(SESSION, &cookie, Some("same-origin")).insert_header(("X-API-Key", "wrong"));
    assert_eq!(status(&app, req).await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
#[serial]
async fn logout_ends_the_session_and_clears_the_cookie() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let app = app!(store);
    let cookie = cookie_pair(&login(&app).await);
    let req = test::TestRequest::delete()
        .uri(SESSION)
        .insert_header((header::COOKIE, cookie.clone()))
        .insert_header(("Sec-Fetch-Site", "same-origin"))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let cleared = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cleared.contains("Max-Age=0"), "{}", cleared);
    assert_eq!(
        status(&app, browser_get(SESSION, &cookie, Some("same-origin"))).await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
#[serial]
async fn expired_sessions_are_refused() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(Duration::from_millis(20));
    let app = app!(store);
    let cookie = cookie_pair(&login(&app).await);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        status(&app, browser_get(SESSION, &cookie, Some("same-origin"))).await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
#[serial]
async fn a_cookie_is_refused_when_no_session_store_is_registered() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let store = sessions(TTL);
    let with_store = app!(store);
    let cookie = cookie_pair(&login(&with_store).await);
    let without_store =
        test::init_service(App::new().configure(rustymail::dashboard::api::init_routes)).await;
    assert_eq!(
        status(
            &without_store,
            browser_get(SESSION, &cookie, Some("same-origin"))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
#[serial]
async fn login_is_refused_when_no_key_is_configured() {
    std::env::remove_var("RUSTYMAIL_API_KEY");
    let store = sessions(TTL);
    let app = app!(store);
    let req = test::TestRequest::post()
        .uri(SESSION)
        .set_json(serde_json::json!({ "api_key": "" }));
    let result = status(&app, req).await;
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    assert_eq!(result, StatusCode::INTERNAL_SERVER_ERROR);
}
