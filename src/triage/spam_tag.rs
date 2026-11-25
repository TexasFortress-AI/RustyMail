// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The tag spam filters write into a subject ("***SPAM***", "*****SPAM*****",
//! "[SPAM]"). A rescued message is not spam, so its subject loses the tag.

/// The subject without a leading spam tag, or None when it has none.
pub fn untagged_subject(subject: &str) -> Option<String> {
    let trimmed = subject.trim_start();
    let rest = if let Some(after) = trimmed.strip_prefix('[') {
        let (word, rest) = after.split_once(']')?;
        word.eq_ignore_ascii_case("SPAM").then_some(rest)?
    } else {
        let after_stars = trimmed.trim_start_matches('*');
        let stars = trimmed.len() - after_stars.len();
        let word = after_stars.get(..4)?;
        let rest = after_stars[4..].trim_start_matches('*');
        let closing = after_stars.len() - 4 - rest.len();
        (stars > 0 && closing > 0 && word.eq_ignore_ascii_case("SPAM")).then_some(rest)?
    };
    Some(rest.trim_start().to_string())
}

/// The raw message with the spam tag removed from its Subject header, or
/// None when the header has no tag (or the message has no Subject). Only the
/// header block is read; the body is never touched.
pub fn untag_message(raw: &[u8]) -> Option<Vec<u8>> {
    let header_end = find(raw, b"\r\n\r\n").map(|i| i + 2).unwrap_or(raw.len());
    let mut start = 0;
    while start < header_end {
        let end = find(&raw[start..header_end], b"\r\n").map_or(header_end, |i| start + i + 2);
        let line = &raw[start..end];
        if line.len() > 8 && line[..8].eq_ignore_ascii_case(b"subject:") {
            let value = std::str::from_utf8(&line[8..]).ok()?;
            let clean = untagged_subject(value.trim_end_matches(['\r', '\n']))?;
            let mut out = Vec::with_capacity(raw.len());
            out.extend_from_slice(&raw[..start]);
            out.extend_from_slice(format!("Subject: {}\r\n", clean).as_bytes());
            out.extend_from_slice(&raw[end..]);
            return Some(out);
        }
        start = end;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
