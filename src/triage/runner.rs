// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage one account: find new messages, classify them, record proposals.
//!
//! Read-only on the mailbox: SELECT, UID SEARCH and BODY.PEEK[] fetches, which
//! do not set \Seen. Nothing is moved, flagged or deleted.
//!
//! The first run works through the whole folder, oldest first; the watermark
//! is saved after every batch, so an interrupted run resumes where it stopped
//! and later runs only see UIDs above it.

use super::answered::answered_uids;
use super::classifier::Classifier;
use super::counterparty::counterparty;
use super::destinations::FolderResolver;
use super::folders::FolderMap;
use super::guards::ModelGuards;
use super::lists::ListRules;
use super::message::TriageMessage;
use super::people::{owner_accounts, KnownPeople};
use super::rules::{apply_rules, bulk_mail_default, SenderRules};
use super::scan::{advance, plan_scan, select_uids};
use super::store::{bare_message_id, MessageRef, TriageStore, Watermark};
use super::{Bucket, Proposal, ProposalSource};
use crate::imap::account_connect::ImapAccount;
use crate::imap::client::ImapClient;
use crate::imap::session::AsyncImapSessionWrapper;
use log::{info, warn};
use std::collections::HashSet;

pub type Client = ImapClient<AsyncImapSessionWrapper>;
type RunError = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    pub since_days: Option<u32>,
    pub limit: Option<usize>,
    /// Classify and log, but write no proposals and leave the watermark alone.
    pub dry_run: bool,
}

#[derive(Debug, Default)]
pub struct AccountSummary {
    pub account: String,
    pub examined: usize,
    pub by_rule: usize,
    pub by_model: usize,
    pub unparseable: usize,
    /// Highest UID triaged so far in this folder.
    pub watermark: Option<u32>,
    /// Why the run stopped before the end (model unavailable, etc.).
    pub stopped_early: Option<String>,
    /// Spam-folder review: messages triage itself filed as Spam (they stay
    /// unless the owner corrects them).
    pub filed_by_triage: usize,
}

/// Which folder a pass reads, and what a verdict there means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The INBOX: a proposal says where a message should go from there.
    Inbox,
    /// The spam folder the mail server filled: a Spam verdict, or a verdict
    /// the guards hold for the owner, means the message stays; anything else
    /// proposes rescuing it.
    Junk,
}

/// One pass over one folder.
struct Pass<'a> {
    folder: &'a str,
    role: Role,
    /// Junk only: Message-IDs of mail triage itself filed as Spam.
    filed_as_spam: HashSet<String>,
}

pub struct TriageRunner<C: Classifier> {
    pub store: TriageStore,
    pub classifier: C,
    pub guards: ModelGuards,
    pub senders: SenderRules,
    pub lists: ListRules,
    pub code_window: chrono::Duration,
    pub folder: String,
    pub fetch_batch: usize,
    pub excerpt_chars: usize,
    /// Body characters kept for a second-pass model (at least `excerpt_chars`).
    pub fuller_excerpt_chars: usize,
    /// All the owner's triaged accounts; their Sent folders define who the
    /// owner corresponds with.
    pub owner_accounts: Vec<String>,
    /// Domains of platforms that deliver mail on someone's behalf; for their
    /// mail the counterparty is the person in Reply-To.
    pub platform_domains: Vec<String>,
    /// Review each account's spam folder too (TRIAGE_REVIEW_JUNK), finding
    /// the folder through this map; None turns the review off.
    pub junk_review: Option<FolderMap>,
}

impl<C: Classifier> TriageRunner<C> {
    pub async fn run_account(
        &self,
        account: &ImapAccount,
        options: RunOptions,
    ) -> Result<AccountSummary, RunError> {
        let client = account.connect().await?;
        let result = self
            .triage_folder(&client, &account.email_address, options)
            .await;
        if let Err(e) = client.logout().await {
            warn!("Failed to log out of {}: {}", account.email_address, e);
        }
        result
    }

    /// Review the account's spam folder (the one the server marks \\Junk, or
    /// the configured one), if the review is on and the folder exists.
    pub async fn review_junk(
        &self,
        account: &ImapAccount,
        options: RunOptions,
    ) -> Result<Option<AccountSummary>, RunError> {
        let Some(folders) = &self.junk_review else {
            return Ok(None);
        };
        let account_id = &account.email_address;
        let client = account.connect().await?;
        let result = async {
            let resolver = FolderResolver::new(&client, account_id, folders).await?;
            let Some(junk) = resolver.existing_folder(Bucket::Spam) else {
                info!("{}: no spam folder to review", account_id);
                return Ok(None);
            };
            self.triage_junk(&client, account_id, &junk, options)
                .await
                .map(Some)
        }
        .await;
        if let Err(e) = client.logout().await {
            warn!("Failed to log out of {}: {}", account_id, e);
        }
        result
    }

    /// Review `folder` as a spam folder over an open connection; the entry
    /// point for tests. Mail triage itself filed as Spam is left alone.
    pub async fn triage_junk(
        &self,
        client: &Client,
        account_id: &str,
        folder: &str,
        options: RunOptions,
    ) -> Result<AccountSummary, RunError> {
        let pass = Pass {
            folder,
            role: Role::Junk,
            filed_as_spam: self.store.filed_as_spam(account_id).await?,
        };
        self.triage_pass(client, account_id, &pass, options).await
    }

    /// Triage the INBOX (TRIAGE_FOLDER) over an open connection; the entry point for tests.
    pub async fn triage_folder(
        &self,
        client: &Client,
        account_id: &str,
        options: RunOptions,
    ) -> Result<AccountSummary, RunError> {
        let pass = Pass {
            folder: &self.folder,
            role: Role::Inbox,
            filed_as_spam: HashSet::new(),
        };
        self.triage_pass(client, account_id, &pass, options).await
    }

    async fn triage_pass(
        &self,
        client: &Client,
        account_id: &str,
        pass: &Pass<'_>,
        options: RunOptions,
    ) -> Result<AccountSummary, RunError> {
        let mut summary = AccountSummary {
            account: account_id.to_string(),
            ..Default::default()
        };
        let mailbox = client.select_folder(pass.folder).await?;
        let uidvalidity = mailbox
            .uid_validity
            .ok_or("server did not report UIDVALIDITY")?;
        let stored = self.store.watermark(account_id, pass.folder).await?;
        let mut watermark = stored
            .filter(|w| w.uidvalidity == uidvalidity)
            .map(|w| w.last_uid);
        summary.watermark = watermark;
        let today = chrono::Utc::now().date_naive();
        let plan = plan_scan(stored, uidvalidity, options.since_days, today);

        let found = client.search_emails(&plan.criteria).await?;
        let proposed = self
            .store
            .proposed_uids(account_id, pass.folder, uidvalidity)
            .await?;
        let uids = select_uids(&found, plan.above, &proposed, options.limit);
        let owner = owner_accounts(&self.owner_accounts, account_id);
        let known = KnownPeople::load(self.store.pool(), &owner).await?;
        info!(
            "{} {}: {} message(s) to triage ({})",
            account_id,
            pass.folder,
            uids.len(),
            plan.criteria
        );

        for batch in uids.chunks(self.fetch_batch) {
            let last_done = self
                .triage_batch(
                    client,
                    account_id,
                    pass,
                    uidvalidity,
                    batch,
                    &known,
                    options,
                    &mut summary,
                )
                .await?;
            // A since-days backfill reaches below the watermark; it never moves it back.
            if options.dry_run || options.since_days.is_some() {
                if summary.stopped_early.is_some() {
                    break;
                }
                continue;
            }
            if let Some(last_uid) = advance(watermark, last_done) {
                if Some(last_uid) != watermark {
                    let saved = Watermark {
                        uidvalidity,
                        last_uid,
                    };
                    self.store
                        .set_watermark(account_id, pass.folder, saved)
                        .await?;
                    watermark = Some(last_uid);
                }
            }
            if summary.stopped_early.is_some() {
                break;
            }
        }
        summary.watermark = watermark;
        Ok(summary)
    }

    /// Classify one batch; returns the highest UID finished in order.
    async fn triage_batch(
        &self,
        client: &Client,
        account_id: &str,
        pass: &Pass<'_>,
        uidvalidity: u32,
        batch: &[u32],
        known: &KnownPeople,
        options: RunOptions,
        summary: &mut AccountSummary,
    ) -> Result<Option<u32>, RunError> {
        let mut emails = client.fetch_emails(batch).await?;
        emails.sort_by_key(|e| e.uid);
        let flagged_answered = answered_uids(client, batch).await;
        let mut last_done = None;
        for email in emails {
            summary.examined += 1;
            let parsed = email.body.as_deref().and_then(|raw| {
                TriageMessage::from_raw_with_fuller(
                    email.uid,
                    raw,
                    self.excerpt_chars,
                    self.fuller_excerpt_chars,
                )
            });
            let Some(mut message) = parsed else {
                warn!(
                    "{}: uid {} could not be parsed; skipped",
                    account_id, email.uid
                );
                summary.unparseable += 1;
                last_done = Some(email.uid);
                continue;
            };
            let filed_by_triage = pass.role == Role::Junk
                && message
                    .message_id
                    .as_deref()
                    .map(bare_message_id)
                    .is_some_and(|id| pass.filed_as_spam.contains(&id));
            if email.internal_date.is_some() {
                message.received_at = email.internal_date;
            }
            message.from_correspondent = known.contains(&message.from_address);
            message.new_counterparty = counterparty(&message, &self.platform_domains)
                .is_none_or(|party| !known.deals_with(party));
            message.answered = flagged_answered.contains(&message.uid) || known.answered(&message);
            let proposal = match self.propose(&message, known, pass.role).await {
                // Triage filed it as Spam earlier with the owner's review: today's
                // verdict is shown, but only the owner's correction moves it.
                Ok(p) if filed_by_triage => {
                    summary.filed_by_triage += 1;
                    stays_filed(p)
                }
                Ok(p) => p,
                Err(e) => {
                    summary.stopped_early = Some(e);
                    return Ok(last_done);
                }
            };
            match proposal.source {
                ProposalSource::Rule => summary.by_rule += 1,
                ProposalSource::Model => summary.by_model += 1,
            }
            // No sender, subject or model reason in logs (#70); the report has them.
            info!(
                "{} uid {} -> {} ({}{})",
                account_id,
                message.uid,
                proposal.bucket,
                proposal.source.as_str(),
                proposal
                    .confidence
                    .map(|c| format!(" {:.2}", c))
                    .unwrap_or_default()
            );
            if !options.dry_run {
                self.record(account_id, pass.folder, uidvalidity, &message, &proposal)
                    .await?;
            }
            last_done = Some(message.uid);
        }
        Ok(last_done)
    }

    async fn propose(
        &self,
        message: &TriageMessage,
        known: &KnownPeople,
        role: Role,
    ) -> Result<Proposal, String> {
        let rule = apply_rules(message, &self.senders, self.code_window)
            .or_else(|| self.lists.proposal(message));
        if let Some(rule) = rule {
            return Ok(rule);
        }
        let proposal = self.classifier.classify(message).await?;
        // In the spam folder a held verdict means the message stays there:
        // phishing the server caught is never rescued on a model's word.
        if role == Role::Junk {
            if let Some(hold) = self.guards.hold(&proposal, message) {
                return Ok(Proposal {
                    reason: format!(
                        "stays in spam, {}; model said {} ({})",
                        hold.reason(),
                        proposal.bucket,
                        proposal.reason
                    ),
                    bucket: Bucket::Spam,
                    ..proposal
                });
            }
        }
        let checked = self
            .guards
            .check(proposal, message, known, chrono::Utc::now());
        Ok(bulk_mail_default(message, checked))
    }

    async fn record(
        &self,
        account_id: &str,
        folder: &str,
        uidvalidity: u32,
        message: &TriageMessage,
        proposal: &Proposal,
    ) -> Result<(), sqlx::Error> {
        let reference = MessageRef {
            account_id,
            folder,
            uidvalidity,
            uid: message.uid,
            message_id: message.message_id.as_deref(),
            from_address: &message.from_address,
            subject: &message.subject,
            counterparty: counterparty(message, &self.platform_domains),
        };
        self.store
            .insert_proposal(&reference, proposal)
            .await
            .map(|_| ())
    }
}

/// A message triage itself filed as Spam: it stays, whatever today's verdict,
/// and the reason says what that verdict is so the owner can correct it.
fn stays_filed(proposal: Proposal) -> Proposal {
    if proposal.bucket == Bucket::Spam {
        return proposal;
    }
    Proposal {
        reason: format!(
            "stays in spam, filed by triage earlier; model now says {} ({})",
            proposal.bucket, proposal.reason
        ),
        bucket: Bucket::Spam,
        ..proposal
    }
}
