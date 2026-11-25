// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Stored flag names back to the form IMAP commands take.

use rustymail::imap::flag_changes::wire_flag;

#[test]
fn stored_flag_names_become_imap_flags() {
    assert_eq!(wire_flag("Seen").as_deref(), Some("\\Seen"));
    assert_eq!(wire_flag("Answered").as_deref(), Some("\\Answered"));
    assert_eq!(wire_flag("Custom(\"$Junk\")").as_deref(), Some("$Junk"));
    assert_eq!(wire_flag("Custom(\"NonJunk\")").as_deref(), Some("NonJunk"));
    // A copy never starts out deleted, and \Recent is the server's to set.
    assert_eq!(wire_flag("Deleted"), None);
    assert_eq!(wire_flag("Recent"), None);
    assert_eq!(wire_flag("Garbage"), None);
}
