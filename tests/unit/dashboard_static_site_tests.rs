// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which directory, if any, the backend serves as the dashboard.

use rustymail::config::DashboardConfig;
use rustymail::dashboard::static_site::{
    dashboard_site_dir, DashboardSiteUnavailable, DASHBOARD_INDEX_FILE,
};

fn dashboard(enabled: bool, path: Option<&str>) -> DashboardConfig {
    DashboardConfig { enabled, port: 0, path: path.map(str::to_string) }
}

#[test]
fn not_served_without_dashboard_config_or_when_disabled() {
    let built = tempfile::tempdir().unwrap();
    std::fs::write(built.path().join(DASHBOARD_INDEX_FILE), "<html>").unwrap();
    let path = built.path().to_str().unwrap();

    assert_eq!(dashboard_site_dir(None), Err(DashboardSiteUnavailable::Disabled));
    assert_eq!(
        dashboard_site_dir(Some(&dashboard(false, Some(path)))),
        Err(DashboardSiteUnavailable::Disabled)
    );
}

#[test]
fn not_served_without_a_path() {
    for path in [None, Some(""), Some("  ")] {
        assert_eq!(
            dashboard_site_dir(Some(&dashboard(true, path))),
            Err(DashboardSiteUnavailable::NoPathConfigured)
        );
    }
}

#[test]
fn not_served_until_built() {
    let empty = tempfile::tempdir().unwrap();
    let path = empty.path().to_str().unwrap();
    assert_eq!(
        dashboard_site_dir(Some(&dashboard(true, Some(path)))),
        Err(DashboardSiteUnavailable::NotBuilt(empty.path().to_path_buf()))
    );
}

#[test]
fn served_when_enabled_and_built() {
    let built = tempfile::tempdir().unwrap();
    std::fs::write(built.path().join(DASHBOARD_INDEX_FILE), "<html>").unwrap();
    let path = built.path().to_str().unwrap();
    assert_eq!(
        dashboard_site_dir(Some(&dashboard(true, Some(path)))),
        Ok(built.path().to_path_buf())
    );
}
