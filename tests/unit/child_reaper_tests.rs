// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Spawned sync processes are waited on, so they do not linger as zombies.

use rustymail::dashboard::services::child_reaper::reap_in_background;
use std::process::Command;

/// The test binary itself, asked only to list its tests: exits 0 on every OS.
fn quick_child() -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--list", "--format", "terse"]);
    command.stdout(std::process::Stdio::null());
    command
}

#[test]
fn reaps_the_child_and_reports_its_exit_status() {
    let child = quick_child().spawn().unwrap();
    let status = reap_in_background(child, "Test").join().unwrap();
    assert!(status.expect("waited").success());
}

#[cfg(unix)]
#[test]
fn reports_a_failing_exit_code() {
    let child = Command::new("sh").args(["-c", "exit 3"]).spawn().unwrap();
    let status = reap_in_background(child, "Test").join().unwrap();
    assert_eq!(status.expect("waited").code(), Some(3));
}
