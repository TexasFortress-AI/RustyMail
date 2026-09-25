// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Strict resolution of the `folder` argument.
//!
//! Tool handlers used to read the folder with
//! `params.get("folder").and_then(|v| v.as_str()).unwrap_or("INBOX")`, so a caller that
//! wrote `folder_name` (or `mailbox`, or `folder: ""`, or `folder: 12`) got a confident,
//! complete answer about `INBOX` with no error and no hint that its argument was ignored.
//! That is a wrong-answer bug, not a crash, so it cannot be caught by the caller.
//!
//! These helpers keep the documented default — "Optional. Folder name (default: INBOX)" —
//! for callers that supply no folder key at all, and turn every *attempted-but-misnamed*
//! folder argument into a narrow error that names the key the caller wrote, so the call
//! becomes self-correcting.
//!
//! Constraint: deliberately does **not** reject unrelated unknown parameters. Only
//! folder-shaped keys are diagnosed, because a blanket `deny_unknown_fields` would break
//! unrelated callers that pass extra parameters today.

use serde_json::Value;
use std::fmt;

/// The documented parameter name for "which folder to answer about".
pub const FOLDER_ARG_CANONICAL: &str = "folder";

/// Folder-shaped keys that callers write when they mean `folder`. Compared after
/// normalization (lowercased, `-`/space folded to `_`), so `folderName`, `Folder-Name`,
/// `Mailbox Name` and a mis-cased `Folder` all land here.
const FOLDER_NEAR_MISS_KEYS: &[&str] = &[
    "folder",
    "folder_name",
    "foldername",
    "mailbox",
    "mailbox_name",
    "mailboxname",
    "dir",
];

/// A folder argument the caller clearly intended, but did not name correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderArgError {
    /// A folder-shaped key other than `folder` was supplied and `folder` was absent.
    NearMiss { supplied: String },
    /// `folder` was supplied but is not a usable folder name.
    Invalid { got: &'static str },
}

impl fmt::Display for FolderArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FolderArgError::NearMiss { supplied } => write!(
                f,
                "`{}` is not a parameter of this tool; use `folder` \
                 (optional, defaults to INBOX when omitted)",
                supplied
            ),
            FolderArgError::Invalid { got } => write!(
                f,
                "parameter `folder` must be a non-empty string; got {}. \
                 Omit `folder` entirely to use the default folder (INBOX)",
                got
            ),
        }
    }
}

impl std::error::Error for FolderArgError {}

/// Fold case and separator differences so near-miss keys compare equal.
fn normalize_key(key: &str) -> String {
    key.trim().to_lowercase().replace(['-', ' '], "_")
}

fn is_folder_shaped(normalized: &str) -> bool {
    FOLDER_NEAR_MISS_KEYS.contains(&normalized)
}

/// Deterministically pick one folder-shaped key the caller actually wrote.
fn find_near_miss_key<'a, I: IntoIterator<Item = &'a str>>(keys: I) -> Option<String> {
    let mut hits: Vec<String> = keys
        .into_iter()
        .filter(|key| *key != FOLDER_ARG_CANONICAL && is_folder_shaped(&normalize_key(key)))
        .map(|key| key.to_string())
        .collect();
    hits.sort();
    hits.into_iter().next()
}

/// Human-readable description of a JSON value for the error message.
fn described_value(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "an empty string",
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Resolve the folder an MCP/tool call should answer about.
///
/// * `Ok(Some(folder))` — `folder` was given as a non-empty string.
/// * `Ok(None)` — no folder-shaped key at all; the caller keeps its documented INBOX default.
/// * `Err(NearMiss)` — a folder-shaped key (`folder_name`, `mailbox`, `dir`, …) was given
///   while `folder` was absent.
/// * `Err(Invalid)` — `folder` was given as the wrong type, `null`, or an empty /
///   whitespace-only string.
///
/// Precedence, so behaviour is deterministic when several keys arrive at once: a valid
/// `folder` wins and unrelated keys are ignored; an empty `folder` alongside a near-miss
/// key reports the near-miss (it is the actionable one); a wrong-typed `folder` always
/// reports `Invalid`.
pub fn resolve_folder_arg(params: &Value) -> Result<Option<String>, FolderArgError> {
    let near_miss = params
        .as_object()
        .and_then(|map| find_near_miss_key(map.keys().map(|k| k.as_str())));

    match params.get(FOLDER_ARG_CANONICAL) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        // Empty string plus a near-miss key: point at the key the caller meant.
        Some(Value::String(_)) => match near_miss {
            Some(supplied) => Err(FolderArgError::NearMiss { supplied }),
            None => Err(FolderArgError::Invalid {
                got: "an empty string",
            }),
        },
        Some(other) => Err(FolderArgError::Invalid {
            got: described_value(other),
        }),
        None => match near_miss {
            Some(supplied) => Err(FolderArgError::NearMiss { supplied }),
            None => Ok(None),
        },
    }
}

/// Validate the folder-shaped keys of a raw HTTP query string.
///
/// Handlers that deserialize into a typed `web::Query<T>` struct never see unknown keys,
/// so the near-miss check has to run against `HttpRequest::query_string()` instead of the
/// struct. On `Ok(())` the handler may trust its own `folder: Option<String>` field,
/// including when it is `None` (the caller genuinely omitted the parameter).
///
/// Keys are matched as written: percent-encoded separators (`folder%5Fname`) are not
/// folded into a near-miss.
pub fn check_folder_query_keys(raw_query: &str) -> Result<(), FolderArgError> {
    let mut folder_seen = false;
    let mut folder_empty = false;
    let mut near_miss_keys: Vec<String> = Vec::new();

    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => (pair, ""),
        };
        if key == FOLDER_ARG_CANONICAL {
            folder_seen = true;
            folder_empty = value.trim().is_empty();
            continue;
        }
        if is_folder_shaped(&normalize_key(key)) {
            near_miss_keys.push(key.to_string());
        }
    }

    near_miss_keys.sort();
    near_miss_keys.dedup();

    if folder_seen && !folder_empty {
        return Ok(());
    }
    if let Some(supplied) = near_miss_keys.into_iter().next() {
        return Err(FolderArgError::NearMiss { supplied });
    }
    if folder_empty {
        return Err(FolderArgError::Invalid {
            got: "an empty string",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Case 1: `folder` present and a non-empty string -> use it.
    #[test]
    fn resolves_explicit_folder() {
        assert_eq!(
            resolve_folder_arg(&json!({"folder": "Unsubscribe"})),
            Ok(Some("Unsubscribe".to_string()))
        );
    }

    // Case 2: `folder` absent, near-miss key present -> Err naming both keys.
    #[test]
    fn rejects_near_miss_keys_and_names_them() {
        for key in [
            "folder_name",
            "folderName",
            "Folder-Name",
            "Folder",
            "mailbox",
            "mailbox_name",
            "mailboxName",
            "dir",
        ] {
            let err = resolve_folder_arg(&json!({ key: "Unsubscribe" }))
                .expect_err("near-miss key must not fall through to INBOX");
            let msg = err.to_string();
            assert!(
                msg.contains(&format!("`{}`", key)) && msg.contains("use `folder`"),
                "message for `{}` must name the supplied key and the correct one, got: {}",
                key,
                msg
            );
        }
    }

    #[test]
    fn near_miss_error_survives_extra_unrelated_params() {
        let err = resolve_folder_arg(&json!({"folder_name": "Archive", "limit": 10, "uid": 42}))
            .expect_err("misnamed folder must fail even with valid siblings");
        assert_eq!(
            err,
            FolderArgError::NearMiss {
                supplied: "folder_name".to_string()
            }
        );
    }

    #[test]
    fn unrelated_unknown_params_are_still_accepted() {
        // Deliberate: no blanket deny_unknown_fields.
        assert_eq!(
            resolve_folder_arg(&json!({"subject": "hi", "limit": 5})),
            Ok(None)
        );
    }

    // Case 3: `folder` present but wrong type or empty -> Err.
    #[test]
    fn rejects_empty_and_wrong_typed_folder() {
        for (value, needle) in [
            (json!(""), "empty string"),
            (json!("   "), "empty string"),
            (json!(12), "a number"),
            (json!(true), "a boolean"),
            (json!(null), "null"),
            (json!(["INBOX"]), "an array"),
            (json!({"name": "INBOX"}), "an object"),
        ] {
            let err = match resolve_folder_arg(&json!({"folder": value})) {
                Err(err) => err,
                Ok(ok) => panic!("{} must not resolve to {:?}", value, ok),
            };
            assert!(
                matches!(err, FolderArgError::Invalid { .. })
                    && err.to_string().contains(needle)
                    && err.to_string().contains("`folder`"),
                "unexpected error for {}: {}",
                value,
                err
            );
        }
    }

    // Case 4: no folder-ish key at all -> Ok(None), caller keeps the INBOX default.
    #[test]
    fn omitted_folder_is_ok_none() {
        assert_eq!(resolve_folder_arg(&json!({})), Ok(None));
        assert_eq!(resolve_folder_arg(&json!({"limit": 20})), Ok(None));
        // Non-object params cannot carry a key at all.
        assert_eq!(resolve_folder_arg(&Value::Null), Ok(None));
        assert_eq!(resolve_folder_arg(&json!("INBOX")), Ok(None));
    }

    #[test]
    fn valid_folder_wins_over_near_miss_and_unknown_keys() {
        assert_eq!(
            resolve_folder_arg(&json!({"folder": "Unsubscribe", "folder_name": "Junk"})),
            Ok(Some("Unsubscribe".to_string()))
        );
    }

    #[test]
    fn empty_folder_prefers_the_near_miss_key_message() {
        assert_eq!(
            resolve_folder_arg(&json!({"folder": "", "folder_name": "Unsubscribe"})),
            Err(FolderArgError::NearMiss {
                supplied: "folder_name".to_string()
            })
        );
    }

    #[test]
    fn near_miss_reported_key_is_deterministic() {
        assert_eq!(
            resolve_folder_arg(&json!({"mailbox": "A", "dir": "B", "folder_name": "C"})),
            Err(FolderArgError::NearMiss {
                supplied: "dir".to_string()
            })
        );
    }

    #[test]
    fn folder_name_key_is_only_wrong_for_tools_that_take_folder() {
        // `create_folder` legitimately takes `folder_name`; it never calls the resolver,
        // so its params must not be rejected by this module's contract.
        assert_eq!(FOLDER_ARG_CANONICAL, "folder");
    }

    // Query-string surface (typed web::Query handlers).
    #[test]
    fn query_string_with_folder_is_valid() {
        assert!(check_folder_query_keys("folder=Unsubscribe&limit=10").is_ok());
        assert!(check_folder_query_keys("limit=10&account_id=7").is_ok());
        assert!(check_folder_query_keys("").is_ok());
    }

    #[test]
    fn query_string_with_near_miss_key_is_rejected() {
        let err = check_folder_query_keys("folder_name=Unsubscribe&limit=10")
            .expect_err("misnamed query key must not fall through to INBOX");
        assert_eq!(
            err,
            FolderArgError::NearMiss {
                supplied: "folder_name".to_string()
            }
        );
        assert!(check_folder_query_keys("mailbox=Junk").is_err());
        assert!(check_folder_query_keys("Mailbox-Name=Junk").is_err());
    }

    #[test]
    fn query_string_with_empty_folder_is_rejected() {
        let err = check_folder_query_keys("folder=&limit=5")
            .expect_err("empty folder must not fall through to INBOX");
        assert!(matches!(err, FolderArgError::Invalid { .. }), "{}", err);
        // Documented limitation: values are read as written, so percent-encoded
        // whitespace is not detected as empty here.
        assert!(check_folder_query_keys("folder=%20").is_ok());
    }

    #[test]
    fn query_string_folder_wins_over_near_miss() {
        assert!(check_folder_query_keys("folder=Unsubscribe&folder_name=Junk").is_ok());
    }
}
