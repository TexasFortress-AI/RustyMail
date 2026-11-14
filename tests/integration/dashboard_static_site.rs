// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The backend serves the built dashboard at `/` without shadowing the API
//! or exposing anything outside the build directory.

use actix_web::http::{header, StatusCode};
use actix_web::{test, App};
use rustymail::dashboard::static_site::{configure_dashboard_site, DASHBOARD_INDEX_FILE};
use serial_test::serial;
use std::path::Path;

const TEST_KEY: &str = "dashboard-static-site-test-key";
const INDEX_HTML: &str = "<html>rustymail dashboard</html>";
const ASSET_JS: &str = "console.log('asset');";

/// A build dir (index, asset, hidden file) inside a parent holding a secret.
fn build_fixture() -> tempfile::TempDir {
    let parent = tempfile::tempdir().unwrap();
    let dist = parent.path().join("dist");
    std::fs::create_dir_all(dist.join("assets")).unwrap();
    std::fs::write(dist.join(DASHBOARD_INDEX_FILE), INDEX_HTML).unwrap();
    std::fs::write(dist.join("assets/app.js"), ASSET_JS).unwrap();
    std::fs::write(dist.join(".env"), "SECRET=hidden").unwrap();
    std::fs::write(parent.path().join("secret.txt"), "outside").unwrap();
    parent
}

macro_rules! app {
    ($dist:expr) => {{
        let dist: &Path = $dist;
        test::init_service(
            App::new()
                .configure(rustymail::dashboard::api::init_routes)
                .configure(|cfg| configure_dashboard_site(cfg, dist)),
        )
        .await
    }};
}

async fn get<S, B>(app: &S, uri: &str) -> (StatusCode, Option<String>, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    B: actix_web::body::MessageBody,
{
    let req = test::TestRequest::get().uri(uri).to_request();
    match test::try_call_service(app, req).await {
        Ok(response) => {
            let status = response.status();
            let cache = response
                .headers()
                .get(header::CACHE_CONTROL)
                .map(|v| v.to_str().unwrap().to_string());
            let body = test::read_body(response).await;
            (status, cache, String::from_utf8_lossy(&body).into_owned())
        }
        Err(error) => (error.as_response_error().status_code(), None, String::new()),
    }
}

#[actix_web::test]
async fn root_serves_index_and_assets_with_revalidation() {
    let fixture = build_fixture();
    let app = app!(&fixture.path().join("dist"));

    let (status, cache, body) = get(&app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, INDEX_HTML);
    assert_eq!(cache.as_deref(), Some("no-cache"));

    let (status, _, body) = get(&app, "/assets/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, ASSET_JS);
}

#[actix_web::test]
#[serial]
async fn api_routes_still_win_and_still_require_the_key() {
    std::env::set_var("RUSTYMAIL_API_KEY", TEST_KEY);
    let fixture = build_fixture();
    let app = app!(&fixture.path().join("dist"));

    let (status, _, body) = get(&app, "/api/dashboard/stats").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_ne!(body, INDEX_HTML);
}

#[actix_web::test]
async fn unknown_paths_are_404_not_the_index() {
    let fixture = build_fixture();
    let app = app!(&fixture.path().join("dist"));

    for uri in ["/missing", "/api/v1/emails", "/assets/"] {
        let (status, _, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_ne!(body, INDEX_HTML, "{uri}");
    }
}

#[actix_web::test]
async fn nothing_outside_the_build_or_hidden_is_served() {
    let fixture = build_fixture();
    let app = app!(&fixture.path().join("dist"));

    for uri in ["/../secret.txt", "/%2e%2e/secret.txt", "/assets/..%2f..%2fsecret.txt", "/.env"] {
        let (status, _, body) = get(&app, uri).await;
        assert_ne!(status, StatusCode::OK, "{uri}");
        assert!(!body.contains("outside") && !body.contains("SECRET"), "{uri}");
    }
}
