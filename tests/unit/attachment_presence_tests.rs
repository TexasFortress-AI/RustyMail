// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Attachments that sync recorded as metadata only are not "downloaded".

use chrono::Utc;
use rustymail::dashboard::services::attachment_presence::all_downloaded;
use rustymail::dashboard::services::attachment_storage::AttachmentInfo;

fn attachment(storage_path: &str) -> AttachmentInfo {
    AttachmentInfo {
        filename: "invoice.pdf".into(),
        size_bytes: 10,
        content_type: Some("application/pdf".into()),
        content_id: None,
        downloaded_at: Utc::now(),
        storage_path: storage_path.into(),
    }
}

#[test]
fn metadata_only_rows_still_need_the_imap_fetch() {
    assert!(!all_downloaded(&[]), "no rows: fetch");
    assert!(!all_downloaded(&[attachment("")]), "metadata only: fetch");
    assert!(
        !all_downloaded(&[attachment("/a/b.pdf"), attachment("")]),
        "partly on disk: fetch"
    );
    assert!(
        all_downloaded(&[attachment("/a/b.pdf")]),
        "on disk: serve it"
    );
}
