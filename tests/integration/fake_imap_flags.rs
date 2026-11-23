// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Flags and CONDSTORE (RFC 7162) for the fake IMAP server: `\Seen`, a
//! mod-sequence per message, `SELECT ... (CONDSTORE)` reporting
//! HIGHESTMODSEQ, and `UID FETCH <set> (UID FLAGS) [(CHANGEDSINCE n)]`.
//! "Garbled" messages answer with FLAGS the client cannot parse, as some
//! real servers do.

use super::fake_imap_server::FakeMailbox;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct FlagState {
    /// Whether CAPABILITY advertises CONDSTORE.
    pub condstore: bool,
    /// (folder, uid) pairs carrying \Seen.
    pub seen: HashSet<(String, u32)>,
    /// Mod-sequence of each (folder, uid); missing means 1.
    pub modseq: HashMap<(String, u32), u64>,
    /// (folder, uid) pairs whose FLAGS reply does not parse.
    pub garbled: HashSet<(String, u32)>,
}

impl FlagState {
    /// Marks a message read or unread and gives it the next mod-sequence.
    pub fn set_seen(&mut self, folder: &str, uid: u32, seen: bool) {
        let key = (folder.to_string(), uid);
        if seen {
            self.seen.insert(key.clone());
        } else {
            self.seen.remove(&key);
        }
        let next = self.highest() + 1;
        self.modseq.insert(key, next);
    }

    fn highest(&self) -> u64 {
        self.modseq.values().copied().max().unwrap_or(1)
    }

    fn modseq_of(&self, folder: &str, uid: u32) -> u64 {
        *self.modseq.get(&(folder.to_string(), uid)).unwrap_or(&1)
    }
}

impl FakeMailbox {
    /// The reply to a flags or CONDSTORE command, or None for other commands.
    pub(super) fn flags_reply(
        &mut self,
        selected: &mut String,
        tag: &str,
        command: &str,
    ) -> Option<String> {
        let upper = command.to_uppercase();
        if upper.starts_with("CAPABILITY") && self.flags.condstore {
            return Some(format!(
                "* CAPABILITY IMAP4rev1 MOVE UIDPLUS CONDSTORE\r\n{} OK done\r\n",
                tag
            ));
        }
        if upper.starts_with("SELECT ") && upper.ends_with(" (CONDSTORE)") {
            let name = command[7..command.len() - " (CONDSTORE)".len()]
                .trim()
                .trim_matches('"');
            let messages = self.folders.get(name)?.len();
            *selected = name.to_string();
            return Some(format!(
                "* {} EXISTS\r\n* OK [UIDVALIDITY 1] ok\r\n* OK [UIDNEXT {}] ok\r\n* OK [HIGHESTMODSEQ {}] ok\r\n{} OK [READ-WRITE] done\r\n",
                messages,
                self.next_uid,
                self.flags.highest(),
                tag
            ));
        }
        if upper.starts_with("UID FETCH ") && upper.contains("(UID FLAGS)") {
            return Some(self.fetch_flag_lines(selected, tag, command));
        }
        None
    }

    fn fetch_flag_lines(&self, selected: &str, tag: &str, command: &str) -> String {
        let set = command.split_whitespace().nth(2).unwrap();
        let since = command
            .to_uppercase()
            .split("CHANGEDSINCE ")
            .nth(1)
            .and_then(|rest| rest.trim_end_matches(')').trim().parse::<u64>().ok());
        let wanted: Option<HashSet<u32>> =
            (set != "1:*").then(|| set.split(',').map(|uid| uid.parse().unwrap()).collect());
        let mut out = String::new();
        for (seq, message) in self.folders[selected].iter().enumerate() {
            let key = (selected.to_string(), message.uid);
            let modseq = self.flags.modseq_of(selected, message.uid);
            if wanted.as_ref().is_some_and(|w| !w.contains(&message.uid))
                || since.is_some_and(|since| modseq <= since)
            {
                continue;
            }
            let flags = if self.flags.garbled.contains(&key) {
                "\\Seen ((".to_string()
            } else if self.flags.seen.contains(&key) {
                "\\Seen".to_string()
            } else {
                String::new()
            };
            let modseq = if self.flags.condstore {
                format!(" MODSEQ ({modseq})")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "* {} FETCH (UID {} FLAGS ({}){})\r\n",
                seq + 1,
                message.uid,
                flags,
                modseq
            ));
        }
        format!("{}{} OK done\r\n", out, tag)
    }
}
