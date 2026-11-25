// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! APPEND for the fake IMAP server: the literal becomes a new message in
//! the folder, and the flags and INTERNALDATE the client sent are recorded.

use super::fake_imap_server::{FakeMailbox, FakeMessage};

/// One APPEND as the server received it.
#[derive(Clone, Debug, PartialEq)]
pub struct Appended {
    pub folder: String,
    pub uid: u32,
    /// The parenthesised flag list, e.g. `(\Seen $NotJunk)`, if one was sent.
    pub flags: Option<String>,
    /// The quoted INTERNALDATE, if one was sent.
    pub internal_date: Option<String>,
    pub message_id: String,
}

/// The literal length announced at the end of a command line (`... {123}`).
pub fn literal_len(line: &str) -> Option<usize> {
    let open = line.rfind('{')?;
    line.strip_suffix('}')?[open + 1..].parse().ok()
}

impl FakeMailbox {
    /// `APPEND "folder" [(flags)] ["date"] {n}` followed by the literal.
    pub fn append(&mut self, tag: &str, command: &str, literal: &str) -> String {
        let mut parts = command.splitn(3, '"');
        let folder = parts.nth(1).unwrap_or_default().to_string();
        let rest = parts.next().unwrap_or_default();
        if !self.folders.contains_key(&folder) {
            return format!("{} NO [TRYCREATE] no such folder\r\n", tag);
        }
        let flags = rest.find('(').and_then(|open| {
            rest[open..]
                .find(')')
                .map(|close| rest[open..=open + close].to_string())
        });
        let after_flags = flags
            .as_ref()
            .map_or(rest, |f| rest.split_once(f.as_str()).unwrap().1);
        let internal_date = after_flags
            .split('"')
            .nth(1)
            .map(|date| format!("\"{}\"", date));
        let message_id = literal
            .lines()
            .find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.eq_ignore_ascii_case("Message-ID")
                    .then(|| value.trim().to_string())
            })
            .unwrap_or_default();
        let uid = self.next_uid;
        self.next_uid += 1;
        self.folders.get_mut(&folder).unwrap().push(FakeMessage {
            uid,
            message_id: message_id.clone(),
        });
        self.raw_bodies
            .insert(message_id.clone(), literal.to_string());
        self.appended.push(Appended {
            folder,
            uid,
            flags,
            internal_date,
            message_id,
        });
        format!("{} OK APPEND completed\r\n", tag)
    }
}
