// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Mailing-list posts and repository notifications. The headers are copied
//! from real messages: a Google Groups post, GitHub notifications (with and
//! without a `mailto:` List-Post), a Substack newsletter and a Mailchimp one.

use rustymail::triage::lists::{list_id, ListRules};
use rustymail::triage::message::TriageMessage;
use rustymail::triage::Bucket;

fn message(list_headers: &str) -> TriageMessage {
    message_from("someone@example.test", list_headers)
}

fn message_from(from: &str, list_headers: &str) -> TriageMessage {
    let raw = format!(
        "From: {from}\r\nTo: me@example.test\r\nSubject: Hello\r\n\
         Date: Fri, 13 Nov 2026 10:00:00 +0000\r\nMessage-ID: <l@example.test>\r\n\
         {list_headers}Content-Type: text/plain; charset=utf-8\r\n\r\nBody"
    );
    TriageMessage::from_raw(1, raw.as_bytes(), 500).unwrap()
}

fn rules() -> ListRules {
    ListRules::new(vec!["github.com".to_string()])
}

fn routed(list_headers: &str) -> Option<(Bucket, String)> {
    rules()
        .proposal(&message(list_headers))
        .map(|p| (p.bucket, p.list_name.unwrap()))
}

#[test]
fn a_google_groups_post_goes_to_its_list() {
    let headers = "Precedence: list\r\n\
        List-ID: <cplusdev.googlegroups.com>\r\n\
        List-Post: <https://groups.google.com/group/cplusdev/post>, <mailto:cplusdev@googlegroups.com>\r\n";
    assert_eq!(
        routed(headers),
        Some((Bucket::Lists, "cplusdev".to_string()))
    );
}

#[test]
fn a_mailman_post_goes_to_its_list() {
    let headers = "Precedence: list\r\n\
        List-Id: ZeroMQ development list <zeromq-dev.lists.zeromq.org>\r\n\
        List-Post: <mailto:zeromq-dev@lists.zeromq.org>\r\n";
    assert_eq!(
        routed(headers),
        Some((Bucket::Lists, "zeromq-dev".to_string()))
    );
}

#[test]
fn every_notification_of_a_repo_goes_to_that_repo() {
    // GitHub sends List-Post with mailto:, without it, or not at all.
    for post in [
        "List-Post: <mailto:noreply@github.com>\r\n",
        "List-Post: noreply@github.com\r\n",
        "",
    ] {
        let headers = format!(
            "Precedence: list\r\n\
             List-Id: TexasFortress-AI/RustyMail <RustyMail.TexasFortress-AI.github.com>\r\n{post}"
        );
        assert_eq!(
            routed(&headers),
            Some((Bucket::Repos, "RustyMail".to_string())),
            "{post}"
        );
    }
}

#[test]
fn newsletters_are_not_lists() {
    let substack = "List-Id: <genflynn.substack.com>\r\n\
        List-Post: <https://genflynn.substack.com/p/an-article>\r\n";
    let mailchimp = "List-ID: 5cd1c89a0fa69cb2bc8a6a23emc list \
        <5cd1c89a0fa69cb2bc8a6a23e.546022.list-id.mcsv.net>\r\n";
    let list_admin = "Precedence: bulk\r\n\
        List-Id: ZeroMQ development list <zeromq-dev.lists.zeromq.org>\r\n";
    for headers in [substack, mailchimp, list_admin, ""] {
        assert_eq!(routed(headers), None, "{headers}");
    }
}

#[test]
fn repo_domains_come_from_configuration() {
    let gitlab = "List-Id: <widget.team.gitlab.example>\r\n";
    assert_eq!(routed(gitlab), None);
    let configured = ListRules::new(vec![" .gitlab.example ".to_string()]);
    let proposal = configured.proposal(&message(gitlab)).unwrap();
    assert_eq!(proposal.bucket, Bucket::Repos);
    assert_eq!(proposal.list_name.as_deref(), Some("widget"));
    // A look-alike domain is not the code host.
    assert_eq!(routed("List-Id: <x.notgithub.com>\r\n"), None);
}

#[test]
fn the_folder_name_is_cut_to_safe_characters() {
    let hostile = "List-Id: <Ev/il*%name.lists.example.test>\r\n\
        List-Post: <mailto:list@lists.example.test>\r\n";
    assert_eq!(
        routed(hostile),
        Some((Bucket::Lists, "Evilname".to_string()))
    );
    let climbing = "List-Id: <../etc.lists.example.test>\r\n\
        List-Post: <mailto:list@lists.example.test>\r\n";
    assert_eq!(routed(climbing), None);
    let long = format!(
        "List-Id: <{}.lists.example.test>\r\nList-Post: <mailto:l@lists.example.test>\r\n",
        "a".repeat(200)
    );
    assert_eq!(routed(&long).unwrap().1.len(), 64);
    let nothing_left = "List-Id: <***.lists.example.test>\r\n\
        List-Post: <mailto:l@lists.example.test>\r\n";
    assert_eq!(routed(nothing_left), None);
}

#[test]
fn the_list_id_is_read_from_inside_the_brackets() {
    let m = message("List-Id: ZeroMQ development list <zeromq-dev.lists.zeromq.org>\r\n");
    assert_eq!(list_id(&m).as_deref(), Some("zeromq-dev.lists.zeromq.org"));
    let bare = message("List-Id: zeromq-dev.lists.zeromq.org\r\n");
    assert_eq!(
        list_id(&bare).as_deref(),
        Some("zeromq-dev.lists.zeromq.org")
    );
}

#[test]
fn only_rules_may_choose_lists_repos_and_trash() {
    assert!(Bucket::Lists.rule_only());
    assert!(Bucket::Repos.rule_only());
    assert!(Bucket::Trash.rule_only());
    assert!(!Bucket::Noise.rule_only());
    assert!(rustymail::triage::model::parse_verdict(
        r#"{"bucket": "Lists", "confidence": 0.9, "reason": "x"}"#
    )
    .is_err());
}

#[test]
fn an_organisation_writing_to_its_own_group_is_not_a_list() {
    // A Google Workspace group: a colleague posting to the company-wide list.
    let company = "Precedence: list\r\n\
        List-ID: <all-users.company.example>\r\n\
        List-Post: <https://groups.google.com/a/company.example/group/all-users/post>, \
        <mailto:all-users@company.example>\r\n";
    let colleague = message_from("billy@company.example", company);
    assert!(rules().proposal(&colleague).is_none());
    // The same group posted to from outside the organisation is a list.
    let outsider = message_from("vendor@elsewhere.example", company);
    assert_eq!(rules().proposal(&outsider).unwrap().bucket, Bucket::Lists);
    // A project's own people posting to its list host are still list posts:
    // the list lives on lists.zeromq.org, they write from zeromq.org.
    let maintainer = message_from(
        "maintainer@zeromq.org",
        "List-Id: <zeromq-dev.lists.zeromq.org>\r\n\
         List-Post: <mailto:zeromq-dev@lists.zeromq.org>\r\n",
    );
    assert_eq!(rules().proposal(&maintainer).unwrap().bucket, Bucket::Lists);
}
