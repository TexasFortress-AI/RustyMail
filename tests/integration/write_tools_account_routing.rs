// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Proves EmailService write operations reach the named account's IMAP server
//! (never the default session), and that moves are verified before reporting success.
//! Each account gets a real TCP fake IMAP server, so routing is exercised end to end.

use rustymail::connection_pool::{ConnectionPool, ImapConnectionFactory, PoolConfig};
use rustymail::dashboard::services::account::{Account, AccountService};
use rustymail::dashboard::services::email::EmailService;
use rustymail::imap::error::ImapError;
use rustymail::imap::CloneableImapSessionFactory;
use serial_test::serial;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex as TokioMutex;
use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

const ACCOUNT_A: &str = "default-box@example.test";
const ACCOUNT_B: &str = "named-box@example.test";

/// How the fake server reacts to UID MOVE, to model servers that report OK without moving.
#[derive(Clone, Copy, PartialEq)]
enum MoveBehavior {
    Normal,
    IgnoreMove,
    LoseMessage,
    RefuseCopyAndMove,
}

#[derive(Clone)]
struct FakeMessage {
    uid: u32,
    message_id: String,
}

struct FakeMailbox {
    folders: HashMap<String, Vec<FakeMessage>>,
    commands: Vec<String>,
    connections: usize,
    next_uid: u32,
    move_behavior: MoveBehavior,
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
        if upper.starts_with("UID MOVE ") {
            return self.move_message(selected, tag, command);
        }
        if upper.starts_with("LOGOUT") {
            return format!("* BYE bye\r\n{} OK done\r\n", tag);
        }
        if upper.starts_with("CAPABILITY") {
            return format!("* CAPABILITY IMAP4rev1 MOVE\r\n{} OK done\r\n", tag);
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

type SharedMailbox = Arc<Mutex<FakeMailbox>>;

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

async fn spawn_fake_imap(messages: Vec<(u32, &str)>) -> (u16, SharedMailbox) {
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

fn fake_account(email: &str, port: u16) -> Account {
    Account {
        email_address: email.to_string(),
        id: email.to_string(),
        display_name: email.to_string(),
        provider_type: None,
        imap_host: "127.0.0.1".to_string(),
        imap_port: port as i64,
        imap_user: email.to_string(),
        imap_pass: "pw".to_string(),
        imap_use_tls: true,
        imap_use_starttls: false,
        smtp_host: None,
        smtp_port: None,
        smtp_user: None,
        smtp_pass: None,
        smtp_use_tls: None,
        smtp_use_starttls: None,
        is_active: true,
        is_default: false,
        connection_status: None,
        oauth_provider: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
        oauth_token_expiry: None,
    }
}

/// Two accounts on separate fake servers, plus a default session that must never be used.
struct RoutingHarness {
    service: EmailService,
    box_a: SharedMailbox,
    box_b: SharedMailbox,
    default_session_uses: Arc<AtomicUsize>,
    _config_dir: TempDir,
}

async fn harness(messages_in_b: Vec<(u32, &str)>) -> RoutingHarness {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let config_dir = TempDir::new().unwrap();
    let config_path = config_dir.path().join("accounts.json");
    let db_path = config_dir.path().join("accounts.db");
    std::fs::File::create(&db_path).unwrap();
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let (port_a, box_a) = spawn_fake_imap(vec![(1, "<a-only@example.test>")]).await;
    let (port_b, box_b) = spawn_fake_imap(messages_in_b).await;

    let mut accounts = AccountService::new(config_path.to_str().unwrap());
    accounts.initialize(pool).await.unwrap();
    accounts
        .create_account(fake_account(ACCOUNT_A, port_a))
        .await
        .unwrap();
    accounts
        .create_account(fake_account(ACCOUNT_B, port_b))
        .await
        .unwrap();

    let default_session_uses = Arc::new(AtomicUsize::new(0));
    let uses = default_session_uses.clone();
    let imap_factory = CloneableImapSessionFactory::new(Box::new(move || {
        uses.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ImapError::Connection("default session used".to_string())) })
    }));
    let pool_factory = Arc::new(ImapConnectionFactory::new(
        "127.0.0.1".to_string(),
        1,
        "unused".to_string(),
        "unused".to_string(),
    ));
    let connection_pool = ConnectionPool::new(pool_factory, PoolConfig::default());
    let service = EmailService::new(imap_factory, connection_pool)
        .with_account_service(Arc::new(TokioMutex::new(accounts)));

    RoutingHarness {
        service,
        box_a,
        box_b,
        default_session_uses,
        _config_dir: config_dir,
    }
}

fn uids_in(mailbox: &SharedMailbox, folder: &str) -> Vec<u32> {
    mailbox.lock().unwrap().folders[folder]
        .iter()
        .map(|m| m.uid)
        .collect()
}

fn sent_move(mailbox: &SharedMailbox) -> bool {
    mailbox
        .lock()
        .unwrap()
        .commands
        .iter()
        .any(|c| c.starts_with("UID MOVE"))
}

fn assert_only_b_used(h: &RoutingHarness) {
    assert_eq!(h.default_session_uses.load(Ordering::SeqCst), 0);
    assert_eq!(h.box_a.lock().unwrap().connections, 0, "account A touched");
    assert!(h.box_b.lock().unwrap().connections > 0, "account B unused");
}

#[tokio::test]
#[serial]
async fn move_reaches_only_the_named_account_and_is_verified() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let destination_verified = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();

    assert!(destination_verified);
    assert_only_b_used(&h);
    assert!(uids_in(&h.box_b, "INBOX").is_empty());
    assert_eq!(uids_in(&h.box_b, "Archive"), vec![9000]);
    assert_eq!(
        uids_in(&h.box_a, "INBOX"),
        vec![1],
        "same UID in A untouched"
    );
}

#[tokio::test]
#[serial]
async fn unknown_account_is_rejected_without_connecting() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", "nobody@example.test")
        .await;

    assert!(result.is_err());
    assert_eq!(h.default_session_uses.load(Ordering::SeqCst), 0);
    assert_eq!(h.box_a.lock().unwrap().connections, 0);
    assert_eq!(h.box_b.lock().unwrap().connections, 0);
}

#[tokio::test]
#[serial]
async fn missing_source_uid_fails_without_sending_move() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let error = h
        .service
        .atomic_move_message_for_account(777, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("777"), "got: {}", error);
    assert!(!sent_move(&h.box_b));
    assert_eq!(uids_in(&h.box_b, "INBOX"), vec![1]);
}

#[tokio::test]
#[serial]
async fn missing_destination_folder_fails_without_sending_move() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "NoSuchFolder", ACCOUNT_B)
        .await;

    assert!(result.is_err());
    assert!(!sent_move(&h.box_b));
    assert_eq!(uids_in(&h.box_b, "INBOX"), vec![1]);
}

#[tokio::test]
#[serial]
async fn ok_response_without_a_move_is_reported_as_failure() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::IgnoreMove;

    let error = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("still in"), "got: {}", error);
}

#[tokio::test]
#[serial]
async fn sender_controlled_message_id_is_never_sent_to_the_server() {
    let hostile = r#"<x"y) OR ALL@example.test>"#;
    let h = harness(vec![(1, hostile)]).await;

    let destination_verified = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();

    assert!(destination_verified);
    assert_eq!(uids_in(&h.box_b, "Archive"), vec![9000]);
    let commands = h.box_b.lock().unwrap().commands.clone();
    assert!(
        !commands.iter().any(|c| c.contains("OR ALL")),
        "{:?}",
        commands
    );
}

#[tokio::test]
#[serial]
async fn newest_unrelated_destination_message_does_not_satisfy_verification() {
    // `UID n:*` returns this message even though it predates the move.
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::LoseMessage;
    h.box_b
        .lock()
        .unwrap()
        .folders
        .get_mut("Archive")
        .unwrap()
        .push(FakeMessage {
            uid: 50,
            message_id: "<m1@example.test>".to_string(),
        });

    let error = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("not found in"), "got: {}", error);
}

#[tokio::test]
#[serial]
async fn batch_move_reports_moved_and_failed_uids() {
    let h = harness(vec![(1, "<m1@example.test>"), (2, "<m2@example.test>")]).await;

    let error = h
        .service
        .atomic_batch_move_for_account(&[1, 777, 2], "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("[1, 2]"), "moved UIDs missing: {}", error);
    assert!(error.contains("777"), "failed UID missing: {}", error);
    assert_only_b_used(&h);
    assert!(uids_in(&h.box_b, "INBOX").is_empty());
    assert_eq!(uids_in(&h.box_b, "Archive").len(), 2);
}

#[tokio::test]
#[serial]
async fn flag_and_expunge_operations_reach_only_the_named_account() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    let s = &h.service;

    s.mark_as_read_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.mark_as_unread_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.mark_as_deleted_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.undelete_messages_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.expunge_for_account("INBOX", ACCOUNT_B).await.unwrap();

    assert_only_b_used(&h);
    let commands = h.box_b.lock().unwrap().commands.clone();
    let stores = commands.iter().filter(|c| c.starts_with("UID STORE"));
    assert_eq!(stores.count(), 4);
    assert!(commands.iter().any(|c| c.starts_with("EXPUNGE")));
}

#[tokio::test]
#[serial]
async fn failed_copy_never_deletes_the_same_uid_in_the_destination() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::RefuseCopyAndMove;
    // UIDs are per-folder: this unrelated message happens to share the source UID.
    h.box_b
        .lock()
        .unwrap()
        .folders
        .get_mut("Archive")
        .unwrap()
        .push(FakeMessage {
            uid: 1,
            message_id: "<unrelated@example.test>".to_string(),
        });

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await;

    assert!(result.is_err());
    let commands = h.box_b.lock().unwrap().commands.clone();
    assert!(
        !commands
            .iter()
            .any(|c| c.starts_with("UID STORE") || c.starts_with("EXPUNGE")),
        "rollback touched the destination: {:?}",
        commands
    );
}
