// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Minimal TLS fake IMAP server: one per test account, so routing is exercised over real TCP.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

/// How the fake server reacts to UID MOVE, to model servers that report OK without moving.
#[derive(Clone, Copy, PartialEq)]
pub enum MoveBehavior {
    Normal,
    IgnoreMove,
    LoseMessage,
    RefuseCopyAndMove,
    /// The server has no MOVE extension, so clients fall back to COPY + EXPUNGE.
    NoMoveCommand,
}

#[derive(Clone)]
pub struct FakeMessage {
    pub uid: u32,
    pub message_id: String,
}

pub struct FakeMailbox {
    pub folders: HashMap<String, Vec<FakeMessage>>,
    pub commands: Vec<String>,
    pub connections: usize,
    pub next_uid: u32,
    pub move_behavior: MoveBehavior,
    /// (folder, uid) pairs carrying the \Deleted flag.
    pub deleted: HashSet<(String, u32)>,
    /// Whether CAPABILITY advertises UIDPLUS (UID EXPUNGE).
    pub uidplus: bool,
}

impl FakeMailbox {
    fn handle(&mut self, selected: &mut String, tag: &str, command: &str) -> String {
        self.commands.push(command.to_string());
        let upper = command.to_uppercase();
        if self.move_behavior == MoveBehavior::RefuseCopyAndMove
            && (upper.starts_with("UID MOVE ") || upper.starts_with("UID COPY "))
        {
            return format!("{} NO refused\r\n", tag);
        }
        if upper.starts_with("SELECT ") {
            return self.select(selected, tag, command);
        }
        if upper.starts_with("UID SEARCH UID ") {
            return self.search_uid(selected, tag, &command[15..]);
        }
        if upper.starts_with("UID FETCH ") {
            return self.fetch_headers(selected, tag, command);
        }
        if upper.starts_with("UID MOVE ") && self.move_behavior == MoveBehavior::NoMoveCommand {
            return format!("{} BAD unknown command\r\n", tag);
        }
        if upper.starts_with("UID MOVE ") {
            return self.move_message(selected, tag, command);
        }
        if upper.starts_with("UID COPY ") {
            return self.copy_messages(selected, tag, command);
        }
        if upper.starts_with("UID STORE ")
            && upper.contains("+FLAGS")
            && upper.contains("\\DELETED")
        {
            let uids = uid_list(command.split_whitespace().nth(2).unwrap());
            self.deleted
                .extend(uids.into_iter().map(|uid| (selected.clone(), uid)));
            return format!("{} OK done\r\n", tag);
        }
        if upper.starts_with("UID SEARCH DELETED") {
            let hits: String = self
                .deleted
                .iter()
                .filter(|(folder, _)| folder == selected)
                .map(|(_, uid)| format!(" {}", uid))
                .collect();
            return format!("* SEARCH{}\r\n{} OK done\r\n", hits, tag);
        }
        if upper.starts_with("UID EXPUNGE ") {
            let only = uid_list(command.split_whitespace().nth(2).unwrap());
            self.expunge(selected, Some(&only));
            return format!("{} OK done\r\n", tag);
        }
        if upper.starts_with("EXPUNGE") {
            self.expunge(selected, None);
            return format!("{} OK done\r\n", tag);
        }
        if upper.starts_with("LOGOUT") {
            return format!("* BYE bye\r\n{} OK done\r\n", tag);
        }
        if upper.starts_with("CAPABILITY") {
            let uidplus = if self.uidplus { " UIDPLUS" } else { "" };
            return format!(
                "* CAPABILITY IMAP4rev1 MOVE{}\r\n{} OK done\r\n",
                uidplus, tag
            );
        }
        format!("{} OK done\r\n", tag)
    }

    fn select(&mut self, selected: &mut String, tag: &str, command: &str) -> String {
        let name = command[7..].trim().trim_matches('"').to_string();
        match self.folders.get(&name) {
            Some(messages) => {
                *selected = name;
                format!(
                    "* {} EXISTS\r\n* 0 RECENT\r\n* FLAGS (\\Seen)\r\n* OK [UIDVALIDITY 1] ok\r\n* OK [UIDNEXT {}] ok\r\n{} OK [READ-WRITE] done\r\n",
                    messages.len(),
                    self.next_uid,
                    tag
                )
            }
            None => format!("{} NO no such mailbox\r\n", tag),
        }
    }

    /// Like real servers, `n:*` includes the newest message even when it is below `n`.
    fn search_uid(&self, selected: &str, tag: &str, uid_text: &str) -> String {
        let messages = &self.folders[selected];
        let hits: Vec<u32> = match uid_text.trim().strip_suffix(":*") {
            Some(start) => {
                let start: u32 = start.parse().unwrap();
                let newest = messages.iter().map(|m| m.uid).max();
                messages
                    .iter()
                    .map(|m| m.uid)
                    .filter(|uid| *uid >= start || Some(*uid) == newest)
                    .collect()
            }
            None => {
                let wanted: u32 = uid_text.trim().parse().unwrap();
                messages
                    .iter()
                    .map(|m| m.uid)
                    .filter(|uid| *uid == wanted)
                    .collect()
            }
        };
        let hits: String = hits.iter().map(|uid| format!(" {}", uid)).collect();
        format!("* SEARCH{}\r\n{} OK done\r\n", hits, tag)
    }

    fn fetch_headers(&self, selected: &str, tag: &str, command: &str) -> String {
        let uid: u32 = command.split_whitespace().nth(2).unwrap().parse().unwrap();
        match self.folders[selected].iter().find(|m| m.uid == uid) {
            Some(message) => {
                let headers = format!("Message-ID: {}\r\n\r\n", message.message_id);
                format!(
                    "* 1 FETCH (UID {} BODY[HEADER] {{{}}}\r\n{})\r\n{} OK done\r\n",
                    uid,
                    headers.len(),
                    headers,
                    tag
                )
            }
            None => format!("{} OK done\r\n", tag),
        }
    }

    /// Real servers answer OK when the UID does not exist, so this does too.
    fn move_message(&mut self, selected: &str, tag: &str, command: &str) -> String {
        let mut parts = command.split_whitespace();
        let uid: u32 = parts.nth(2).unwrap().parse().unwrap();
        let destination = parts.next().unwrap().trim_matches('"').to_string();
        let source = self.folders.get_mut(selected).unwrap();
        let position = source.iter().position(|m| m.uid == uid);
        if let (Some(position), true) = (position, self.move_behavior != MoveBehavior::IgnoreMove) {
            let mut message = source.remove(position);
            if self.move_behavior == MoveBehavior::Normal {
                message.uid = self.next_uid;
                self.next_uid += 1;
                self.folders.get_mut(&destination).unwrap().push(message);
            }
        }
        format!("{} OK done\r\n", tag)
    }
}

impl FakeMailbox {
    fn copy_messages(&mut self, selected: &str, tag: &str, command: &str) -> String {
        let mut parts = command.split_whitespace();
        let uids = uid_list(parts.nth(2).unwrap());
        let destination = parts.next().unwrap().trim_matches('"').to_string();
        let copies: Vec<FakeMessage> = self.folders[selected]
            .iter()
            .filter(|m| uids.contains(&m.uid))
            .cloned()
            .collect();
        for mut message in copies {
            message.uid = self.next_uid;
            self.next_uid += 1;
            self.folders.get_mut(&destination).unwrap().push(message);
        }
        format!("{} OK done\r\n", tag)
    }

    /// Removes \Deleted messages from `selected`; with `only`, just those UIDs.
    fn expunge(&mut self, selected: &str, only: Option<&[u32]>) {
        let deleted = &mut self.deleted;
        self.folders.get_mut(selected).unwrap().retain(|m| {
            let flagged = deleted.contains(&(selected.to_string(), m.uid));
            let targeted = only.is_none_or(|uids| uids.contains(&m.uid));
            if flagged && targeted {
                deleted.remove(&(selected.to_string(), m.uid));
                return false;
            }
            true
        });
    }
}

/// Parses a comma-separated UID set such as `1,4,7`.
fn uid_list(set: &str) -> Vec<u32> {
    set.split(',')
        .map(|uid| uid.trim().parse().unwrap())
        .collect()
}

pub type SharedMailbox = Arc<Mutex<FakeMailbox>>;

/// The client refuses plaintext IMAP, so the fake server terminates real TLS.
fn self_signed_tls_acceptor() -> TlsAcceptor {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let certified = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

pub async fn spawn_fake_imap(messages: Vec<(u32, &str)>) -> (u16, SharedMailbox) {
    let mut folders = HashMap::new();
    let inbox = messages
        .into_iter()
        .map(|(uid, id)| FakeMessage {
            uid,
            message_id: id.to_string(),
        })
        .collect::<Vec<_>>();
    folders.insert("INBOX".to_string(), inbox);
    folders.insert("Archive".to_string(), Vec::new());
    let mailbox = Arc::new(Mutex::new(FakeMailbox {
        folders,
        commands: Vec::new(),
        connections: 0,
        next_uid: 9000,
        move_behavior: MoveBehavior::Normal,
        deleted: HashSet::new(),
        uidplus: true,
    }));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept_mailbox = mailbox.clone();
    let acceptor = self_signed_tls_acceptor();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            accept_mailbox.lock().unwrap().connections += 1;
            let acceptor = acceptor.clone();
            let connection_mailbox = accept_mailbox.clone();
            tokio::spawn(async move {
                if let Ok(tls_stream) = acceptor.accept(stream).await {
                    serve_connection(tls_stream, connection_mailbox).await;
                }
            });
        }
    });
    (port, mailbox)
}

async fn serve_connection(
    stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    mailbox: SharedMailbox,
) {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut lines = BufReader::new(read_half).lines();
    let mut selected = String::new();
    write_half.write_all(b"* OK fake ready\r\n").await.unwrap();
    write_half.flush().await.unwrap();
    while let Ok(Some(line)) = lines.next_line().await {
        let (tag, command) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let reply = mailbox.lock().unwrap().handle(&mut selected, tag, command);
        if write_half.write_all(reply.as_bytes()).await.is_err()
            || write_half.flush().await.is_err()
        {
            break;
        }
    }
}
