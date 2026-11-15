// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Wait for a spawned child process in the background.
//!
//! A child that exits without its parent waiting on it stays a zombie until
//! the parent exits. The server spawns rustymail-sync every few minutes, so
//! each sync must be waited on.

use log::{info, warn};
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;

/// Wait for `child` on a background thread and log how it exited. The handle
/// returns the exit status, or None if waiting failed.
pub fn reap_in_background(mut child: Child, label: &'static str) -> JoinHandle<Option<ExitStatus>> {
    let pid = child.id();
    std::thread::spawn(move || match child.wait() {
        Ok(status) => {
            if status.success() {
                info!("{} process {} finished", label, pid);
            } else {
                warn!("{} process {} exited with {:?}", label, pid, status.code());
            }
            Some(status)
        }
        Err(e) => {
            warn!("Could not wait for {} process {}: {}", label, pid, e);
            None
        }
    })
}
