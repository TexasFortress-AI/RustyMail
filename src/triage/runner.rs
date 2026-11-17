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

use super::classifier::Classifier;
use super::guards::ModelGuards;
use super::lists::ListRules;
use super::message::TriageMessage;
use super::people::KnownPeople;
use super::rules::{apply_rules, bulk_mail_default, SenderRules};
use super::scan::{advance, plan_scan, select_uids};
use super::store::{MessageRef, TriageStore, Watermark};
use super::{Proposal, ProposalSource};
use crate::imap::account_connect::ImapAccount;
use crate::imap::client::ImapClient;
use crate::imap::session::AsyncImapSessionWrapper;
use log::{info, warn};

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

    /// Triage one folder over an open connection; the entry point for tests.
    pub async fn triage_folder(
        &self,
        client: &Client,
        account_id: &str,
        options: RunOptions,
    ) -> Result<AccountSummary, RunError> {
        let mut summary = AccountSummary {
            account: account_id.to_string(),
            ..Default::default()
        };
        let mailbox = client.select_folder(&self.folder).await?;
        let uidvalidity = mailbox
            .uid_validity
            .ok_or("server did not report UIDVALIDITY")?;
        let stored = self.store.watermark(account_id, &self.folder).await?;
        let mut watermark = stored
            .filter(|w| w.uidvalidity == uidvalidity)
            .map(|w| w.last_uid);
        summary.watermark = watermark;
        let today = chrono::Utc::now().date_naive();
        let plan = plan_scan(stored, uidvalidity, options.since_days, today);

        let found = client.search_emails(&plan.criteria).await?;
        let proposed = self
            .store
            .proposed_uids(account_id, &self.folder, uidvalidity)
            .await?;
        let uids = select_uids(&found, plan.above, &proposed, options.limit);
        let known = KnownPeople::load(self.store.pool(), account_id).await?;
        info!(
            "{}: {} message(s) to triage ({})",
            account_id,
            uids.len(),
            plan.criteria
        );

        for batch in uids.chunks(self.fetch_batch) {
            let last_done = self
                .triage_batch(
                    client,
                    account_id,
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
                        .set_watermark(account_id, &self.folder, saved)
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
        uidvalidity: u32,
        batch: &[u32],
        known: &KnownPeople,
        options: RunOptions,
        summary: &mut AccountSummary,
    ) -> Result<Option<u32>, RunError> {
        let mut emails = client.fetch_emails(batch).await?;
        emails.sort_by_key(|e| e.uid);
        let mut last_done = None;
        for email in emails {
            summary.examined += 1;
            let parsed = email
                .body
                .as_deref()
                .and_then(|raw| TriageMessage::from_raw(email.uid, raw, self.excerpt_chars));
            let Some(mut message) = parsed else {
                warn!(
                    "{}: uid {} could not be parsed; skipped",
                    account_id, email.uid
                );
                summary.unparseable += 1;
                last_done = Some(email.uid);
                continue;
            };
            if email.internal_date.is_some() {
                message.received_at = email.internal_date;
            }
            let proposal = match self.propose(&message, known).await {
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
                self.record(account_id, uidvalidity, &message, &proposal)
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
    ) -> Result<Proposal, String> {
        let rule = apply_rules(message, &self.senders, self.code_window)
            .or_else(|| self.lists.proposal(message));
        if let Some(rule) = rule {
            return Ok(rule);
        }
        let proposal = self.classifier.classify(message).await?;
        let checked = self
            .guards
            .check(proposal, message, known, chrono::Utc::now());
        Ok(bulk_mail_default(message, checked))
    }

    async fn record(
        &self,
        account_id: &str,
        uidvalidity: u32,
        message: &TriageMessage,
        proposal: &Proposal,
    ) -> Result<(), sqlx::Error> {
        let reference = MessageRef {
            account_id,
            folder: &self.folder,
            uidvalidity,
            uid: message.uid,
            message_id: message.message_id.as_deref(),
            from_address: &message.from_address,
            subject: &message.subject,
        };
        self.store
            .insert_proposal(&reference, proposal)
            .await
            .map(|_| ())
    }
}
