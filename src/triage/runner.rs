// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage one account: find new messages, classify them, record proposals.
//!
//! Read-only on the mailbox: SELECT, UID SEARCH and BODY.PEEK[] fetches, which
//! do not set \Seen. Nothing is moved, flagged or deleted.

use super::message::TriageMessage;
use super::model::{EscalatingClassifier, ModelBackend};
use super::rules::{apply_rules, SenderRules};
use super::scan::{advance, plan_scan, select_uids, ScanPlan};
use super::store::{MessageRef, TriageStore, Watermark};
use super::{Proposal, ProposalSource};
use crate::imap::account_connect::ImapAccount;
use crate::imap::client::ImapClient;
use crate::imap::session::AsyncImapSessionWrapper;
use log::{info, warn};

type Client = ImapClient<AsyncImapSessionWrapper>;
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
    /// The watermark was just set to the end of the folder; nothing triaged.
    pub initialized: bool,
    pub examined: usize,
    pub by_rule: usize,
    pub by_model: usize,
    pub unparseable: usize,
    /// Why the run stopped before the end (model unavailable, etc.).
    pub stopped_early: Option<String>,
}

pub struct TriageRunner<B: ModelBackend> {
    pub store: TriageStore,
    pub classifier: EscalatingClassifier<B>,
    pub senders: SenderRules,
    pub folder: String,
    pub fetch_batch: usize,
    pub excerpt_chars: usize,
}

impl<B: ModelBackend> TriageRunner<B> {
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

    async fn triage_folder(
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
        let today = chrono::Utc::now().date_naive();

        let (criteria, above) = match plan_scan(stored, uidvalidity, options.since_days, today) {
            ScanPlan::Search { criteria, above } => (criteria, above),
            ScanPlan::Initialize => {
                let uid_next = mailbox.uid_next.ok_or("server did not report UIDNEXT")?;
                let start = Watermark {
                    uidvalidity,
                    last_uid: uid_next.saturating_sub(1),
                };
                if !options.dry_run {
                    self.store
                        .set_watermark(account_id, &self.folder, start)
                        .await?;
                }
                info!(
                    "{}: watermark set at UID {}; mail after it will be triaged",
                    account_id, start.last_uid
                );
                summary.initialized = true;
                return Ok(summary);
            }
        };

        let found = client.search_emails(&criteria).await?;
        let proposed = self
            .store
            .proposed_uids(account_id, &self.folder, uidvalidity)
            .await?;
        let uids = select_uids(&found, above, &proposed, options.limit);
        info!(
            "{}: {} message(s) to triage ({})",
            account_id,
            uids.len(),
            criteria
        );

        let mut last_done = None;
        'batches: for batch in uids.chunks(self.fetch_batch) {
            let mut emails = client.fetch_emails(batch).await?;
            emails.sort_by_key(|e| e.uid);
            for email in emails {
                summary.examined += 1;
                let parsed = email
                    .body
                    .as_deref()
                    .and_then(|raw| TriageMessage::from_raw(email.uid, raw, self.excerpt_chars));
                let Some(message) = parsed else {
                    warn!(
                        "{}: uid {} could not be parsed; skipped",
                        account_id, email.uid
                    );
                    summary.unparseable += 1;
                    last_done = Some(email.uid);
                    continue;
                };
                let proposal = match self.propose(&message).await {
                    Ok(p) => p,
                    Err(e) => {
                        summary.stopped_early = Some(e);
                        break 'batches;
                    }
                };
                match proposal.source {
                    ProposalSource::Rule => summary.by_rule += 1,
                    ProposalSource::Model => summary.by_model += 1,
                }
                info!(
                    "{} uid {} -> {} ({}{}) {} | {}",
                    account_id,
                    message.uid,
                    proposal.bucket,
                    proposal.source.as_str(),
                    proposal
                        .confidence
                        .map(|c| format!(" {:.2}", c))
                        .unwrap_or_default(),
                    message.from_address,
                    proposal.reason
                );
                if !options.dry_run {
                    self.record(account_id, uidvalidity, &message, &proposal)
                        .await?;
                }
                last_done = Some(message.uid);
            }
        }

        if !options.dry_run {
            let current = stored
                .filter(|w| w.uidvalidity == uidvalidity)
                .map(|w| w.last_uid);
            if let Some(last_uid) = advance(current, last_done) {
                self.store
                    .set_watermark(
                        account_id,
                        &self.folder,
                        Watermark {
                            uidvalidity,
                            last_uid,
                        },
                    )
                    .await?;
            }
        }
        Ok(summary)
    }

    async fn propose(&self, message: &TriageMessage) -> Result<Proposal, String> {
        match apply_rules(message, &self.senders) {
            Some(rule) => Ok(rule),
            None => self.classifier.classify(message).await,
        }
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
