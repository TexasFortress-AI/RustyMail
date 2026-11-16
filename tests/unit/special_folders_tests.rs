// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Finding the server's Sent and Drafts folders. The listings are the real
//! LIST results of three servers, attributes in the form
//! `list_folders_hierarchical` records them.

use async_imap::types::NameAttribute;
use rustymail::imap::special_folders::{
    find_special_folder, inbox_namespace, marked_folder, SpecialFolder, SpecialUse,
};
use rustymail::imap::types::Folder;

fn tree(listing: &[(&str, &[&str])]) -> Vec<Folder> {
    Folder::build_hierarchy(
        listing
            .iter()
            .map(|(path, attributes)| {
                (
                    path.to_string(),
                    Some(".".to_string()),
                    attributes.iter().map(|a| a.to_string()).collect(),
                )
            })
            .collect(),
    )
}

/// A server that keeps its own folders at the top level, next to the
/// INBOX.Sent and INBOX.Drafts a mail client created earlier.
fn top_level_server() -> Vec<Folder> {
    tree(&[
        ("Deleted Items", &["Trash"]),
        ("Drafts", &["Drafts"]),
        ("INBOX", &[]),
        ("INBOX.Drafts", &[]),
        ("INBOX.Outbox", &[]),
        ("INBOX.Sent", &[]),
        ("Junk Mail", &["Junk"]),
        ("Sent Items", &["Sent"]),
    ])
}

/// A server that keeps everything under the INBOX.
fn inbox_namespace_server() -> Vec<Folder> {
    tree(&[
        ("INBOX", &["Extension(\"\\\\HasChildren\")"]),
        (
            "INBOX.Sent",
            &["Extension(\"\\\\HasNoChildren\")", "Unmarked", "Sent"],
        ),
        (
            "INBOX.Drafts",
            &["Extension(\"\\\\HasNoChildren\")", "Unmarked", "Drafts"],
        ),
        ("INBOX.Outbox", &["Extension(\"\\\\HasNoChildren\")"]),
    ])
}

fn existing(name: &str) -> SpecialFolder {
    SpecialFolder::Existing(name.to_string())
}

#[test]
fn the_attribute_wins_over_an_older_folder_with_the_usual_name() {
    let folders = top_level_server();
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Sent),
        existing("Sent Items")
    );
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Drafts),
        existing("Drafts")
    );
}

#[test]
fn nested_folders_carrying_the_attribute_are_found() {
    let folders = inbox_namespace_server();
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Sent),
        existing("INBOX.Sent")
    );
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Drafts),
        existing("INBOX.Drafts")
    );
}

#[test]
fn without_attributes_the_usual_names_are_tried_in_order() {
    let folders = tree(&[("INBOX", &[]), ("INBOX.Sent", &[]), ("Sent Items", &[])]);
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Sent),
        existing("Sent Items")
    );
    let lowercase = tree(&[("INBOX", &[]), ("sent", &[])]);
    assert_eq!(
        find_special_folder(&lowercase, SpecialUse::Sent),
        existing("sent")
    );
}

#[test]
fn a_missing_folder_is_created_in_the_accounts_namespace() {
    let nested = tree(&[("INBOX", &[]), ("INBOX.Archive", &[])]);
    assert_eq!(
        find_special_folder(&nested, SpecialUse::Sent),
        SpecialFolder::Create("INBOX.Sent".to_string())
    );
    let top_level = tree(&[("INBOX", &[]), ("Archive", &[])]);
    assert_eq!(
        find_special_folder(&top_level, SpecialUse::Drafts),
        SpecialFolder::Create("Drafts".to_string())
    );
}

#[test]
fn unselectable_folders_are_never_chosen() {
    let folders = tree(&[
        ("INBOX", &[]),
        ("Sent", &["NoSelect", "Sent"]),
        ("Sent Items", &[]),
    ]);
    assert!(
        !folders
            .iter()
            .find(|f| f.name == "Sent")
            .unwrap()
            .selectable
    );
    assert_eq!(
        find_special_folder(&folders, SpecialUse::Sent),
        existing("Sent Items")
    );
}

#[test]
fn attribute_strings_match_what_the_imap_library_produces() {
    // list_folders_hierarchical records format!("{:?}", attribute). If a
    // library update changes these, Sent and Drafts would silently fall back
    // to name matching.
    assert_eq!(format!("{:?}", NameAttribute::Sent), "Sent");
    assert_eq!(format!("{:?}", NameAttribute::Drafts), "Drafts");
    assert_eq!(format!("{:?}", NameAttribute::Trash), "Trash");
    assert_eq!(format!("{:?}", NameAttribute::Junk), "Junk");
    assert_eq!(format!("{:?}", NameAttribute::NoSelect), "NoSelect");
}

#[test]
fn marked_folder_is_only_the_folder_carrying_the_attribute() {
    let folders = top_level_server();
    assert_eq!(
        marked_folder(&folders, SpecialUse::Trash).as_deref(),
        Some("Deleted Items")
    );
    assert_eq!(
        marked_folder(&folders, SpecialUse::Junk).as_deref(),
        Some("Junk Mail")
    );
    let unmarked = tree(&[("INBOX", &[]), ("INBOX.Trash", &[]), ("Trash", &[])]);
    assert_eq!(marked_folder(&unmarked, SpecialUse::Trash), None);
}

/// Gmail's real LIST: `/` delimiter, a stray top-level label named
/// "INBOX.Outbox", and its special folders under [Gmail].
fn gmail(extra: &[&str]) -> Vec<Folder> {
    let mut listing: Vec<(String, Option<String>, Vec<String>)> = [
        ("INBOX", vec![]),
        ("INBOX.Outbox", vec![]),
        ("[Gmail]", vec!["NoSelect"]),
        ("[Gmail]/Sent Mail", vec!["Sent"]),
        ("[Gmail]/Trash", vec!["Trash"]),
    ]
    .into_iter()
    .map(|(path, attributes)| {
        (
            path.to_string(),
            Some("/".to_string()),
            attributes.into_iter().map(String::from).collect(),
        )
    })
    .collect();
    listing.extend(
        extra
            .iter()
            .map(|path| (path.to_string(), Some("/".to_string()), Vec::new())),
    );
    Folder::build_hierarchy(listing)
}

#[test]
fn the_namespace_comes_from_real_children_of_the_inbox() {
    assert_eq!(inbox_namespace(&inbox_namespace_server()), "INBOX.");
    assert_eq!(inbox_namespace(&top_level_server()), "INBOX.");
    assert_eq!(
        inbox_namespace(&gmail(&[])),
        "",
        "a label named INBOX.Outbox is not inside the INBOX on Gmail"
    );
    assert_eq!(inbox_namespace(&gmail(&["INBOX/Projects"])), "INBOX/");
    assert_eq!(
        inbox_namespace(&tree(&[("INBOX", &[]), ("Archive", &[])])),
        ""
    );
}

#[test]
fn a_missing_folder_on_gmail_is_created_at_the_top_level() {
    assert_eq!(
        find_special_folder(&gmail(&[]), SpecialUse::Drafts),
        SpecialFolder::Create("Drafts".to_string())
    );
    assert_eq!(
        find_special_folder(&gmail(&[]), SpecialUse::Sent),
        SpecialFolder::Existing("[Gmail]/Sent Mail".to_string())
    );
}
