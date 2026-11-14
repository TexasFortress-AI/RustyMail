// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Serves the built dashboard (`npm run build` output) from the backend, so
//! the page and `/api` share one origin and no proxy sits in between.

use std::path::{Path, PathBuf};

use actix_files::Files;
use actix_web::http::header::{HeaderValue, CACHE_CONTROL};
use actix_web::middleware::DefaultHeaders;
use actix_web::web;

use crate::config::DashboardConfig;

/// The file the browser gets for `/`.
pub const DASHBOARD_INDEX_FILE: &str = "index.html";
/// Revalidate every file on each load (cheap 304s via ETag), so a rebuild
/// never leaves a stale `index.html` pointing at deleted assets.
const DASHBOARD_CACHE_CONTROL: &str = "no-cache";

/// Why the dashboard is not being served, for the startup log.
#[derive(Debug, PartialEq, Eq)]
pub enum DashboardSiteUnavailable {
    Disabled,
    NoPathConfigured,
    NotBuilt(PathBuf),
}

/// Returns the directory to serve when the dashboard is enabled and built.
pub fn dashboard_site_dir(
    dashboard: Option<&DashboardConfig>,
) -> Result<PathBuf, DashboardSiteUnavailable> {
    let dashboard = match dashboard {
        Some(d) if d.enabled => d,
        _ => return Err(DashboardSiteUnavailable::Disabled),
    };
    let dir = match dashboard.path.as_deref() {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p.trim()),
        _ => return Err(DashboardSiteUnavailable::NoPathConfigured),
    };
    if dir.join(DASHBOARD_INDEX_FILE).is_file() {
        Ok(dir)
    } else {
        Err(DashboardSiteUnavailable::NotBuilt(dir))
    }
}

/// Mounts the site at `/`. Register it after every other service: it only
/// answers paths nothing else matched, and unknown paths get a 404.
pub fn configure_dashboard_site(cfg: &mut web::ServiceConfig, dir: &Path) {
    cfg.service(
        web::scope("")
            .wrap(DefaultHeaders::new().add((
                CACHE_CONTROL,
                HeaderValue::from_static(DASHBOARD_CACHE_CONTROL),
            )))
            .service(Files::new("/", dir).index_file(DASHBOARD_INDEX_FILE)),
    );
}
