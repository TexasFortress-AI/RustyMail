// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! rustymail-triage: sort INBOX mail into Spam, Noise, Accounting, Legal, Credit, Alerts,
//! Needs Reply and (expired codes only) Trash.
//!
//!   rustymail-triage                 propose buckets for new mail, then print the report
//!   rustymail-triage --report        print the report only
//!   rustymail-triage --apply         move stored proposals, verifying every move
//!
//! Proposing reads the mailbox without changing it. Only `--apply` moves mail,
//! and it never deletes or expunges anything but the moved messages.

use clap::Parser;
use log::error;
use rustymail::imap::account_connect::{load_active_accounts, ImapAccount};
use rustymail::triage::answered::release_answered;
use rustymail::triage::apply::{apply_account, ApplyOptions};
use rustymail::triage::classifier::ConfiguredClassifier;
use rustymail::triage::config::TriageConfig;
use rustymail::triage::decision::{DecisionClassifier, LocalDecisionServer};
use rustymail::triage::folders::FolderMap;
use rustymail::triage::guards::ModelGuards;
use rustymail::triage::lists::ListRules;
use rustymail::triage::model::{EscalatingClassifier, LocalModelServer};
use rustymail::triage::people::KnownPeople;
use rustymail::triage::report::{format_apply_summary, format_report};
use rustymail::triage::rules::SenderRules;
use rustymail::triage::runner::{AccountSummary, RunOptions, TriageRunner};
use rustymail::triage::second_pass::SecondPass;
use rustymail::triage::store::TriageStore;
use sqlx::SqlitePool;

#[derive(Parser)]
#[command(
    name = "rustymail-triage",
    about = "Propose triage buckets for mail; --apply moves them"
)]
struct Cli {
    #[arg(
        long,
        env = "CACHE_DATABASE_URL",
        default_value = "sqlite:data/email_cache.db"
    )]
    database_url: String,

    /// Account(s) to work on; defaults to TRIAGE_ACCOUNTS.
    #[arg(long)]
    account: Vec<String>,

    /// Triage messages received in the last N days, below the watermark too.
    #[arg(long)]
    since_days: Option<u32>,

    /// At most N messages per account (proposing: oldest first; applying: moves).
    #[arg(long)]
    limit: Option<usize>,

    /// Classify and log only: write no proposals, leave watermarks alone.
    #[arg(long, conflicts_with_all = ["apply", "report"])]
    dry_run: bool,

    /// Print the report only.
    #[arg(long, conflicts_with = "apply")]
    report: bool,

    /// Move stored proposals to their folders, verifying each move.
    #[arg(long)]
    apply: bool,

    /// With --apply: model proposals below this confidence stay in the INBOX
    /// (default TRIAGE_MIN_CONFIDENCE). Rule proposals always qualify.
    #[arg(long, requires = "apply")]
    min_confidence: Option<f64>,

    /// Maximum proposal rows in the report (counts are always complete).
    #[arg(long, default_value_t = 500)]
    report_limit: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let cli = Cli::parse();
    let config = TriageConfig::from_env()?;

    let pool = SqlitePool::connect(&cli.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let store = TriageStore::new(pool.clone());

    let accounts: Vec<String> = if cli.account.is_empty() {
        config.accounts.clone()
    } else {
        cli.account
            .iter()
            .map(|a| a.trim().to_ascii_lowercase())
            .collect()
    };
    let report_filter = (accounts.len() == 1).then(|| accounts[0].as_str());

    if cli.report {
        print_report(&store, report_filter, cli.report_limit, &config).await?;
        return Ok(());
    }
    if accounts.is_empty() {
        return Err("no accounts: pass --account or set TRIAGE_ACCOUNTS".into());
    }

    let failed = if cli.apply {
        apply_all(&pool, &store, &accounts, &cli, &config).await?
    } else {
        let failed = propose_all(&pool, store, &accounts, &cli, &config).await?;
        if !cli.dry_run {
            let store = TriageStore::new(pool.clone());
            print_report(&store, report_filter, cli.report_limit, &config).await?;
        }
        failed
    };
    if failed {
        std::process::exit(1);
    }
    Ok(())
}

async fn print_report(
    store: &TriageStore,
    account: Option<&str>,
    limit: u32,
    config: &TriageConfig,
) -> Result<(), sqlx::Error> {
    let counts = store.bucket_counts(account).await?;
    let proposals = store.pending_proposals(account, limit).await?;
    print!(
        "{}",
        format_report(&counts, &proposals, &config.dashboard_url)
    );
    Ok(())
}

async fn active_account(pool: &SqlitePool, email: &str) -> Option<ImapAccount> {
    match load_active_accounts(pool, Some(email)).await {
        Ok(accounts) => accounts.into_iter().next(),
        Err(e) => {
            error!("{}: cannot load account: {}", email, e);
            None
        }
    }
}

/// Returns whether any account failed or stopped early.
async fn propose_all(
    pool: &SqlitePool,
    store: TriageStore,
    accounts: &[String],
    cli: &Cli,
    config: &TriageConfig,
) -> Result<bool, Box<dyn std::error::Error>> {
    let classifier = build_classifier(config)?;
    let senders = match &config.sender_rules_file {
        Some(path) => SenderRules::load(path)?,
        None => SenderRules::default(),
    };
    let runner = TriageRunner {
        store,
        classifier,
        guards: ModelGuards {
            needs_reply_max_age: config.needs_reply_max_age,
        },
        senders,
        lists: ListRules::new(config.repo_list_domains.clone()),
        code_window: config.code_window,
        folder: config.folder.clone(),
        fetch_batch: config.fetch_batch,
        excerpt_chars: config.excerpt_chars,
        fuller_excerpt_chars: config.second_pass_excerpt_chars,
    };
    let options = RunOptions {
        since_days: cli.since_days,
        limit: cli.limit,
        dry_run: cli.dry_run,
    };
    let mut failed = false;
    for email in accounts {
        let Some(account) = active_account(pool, email).await else {
            error!("{}: not an active account", email);
            failed = true;
            continue;
        };
        match runner.run_account(&account, options).await {
            Ok(summary) => {
                eprintln!("{}", describe_run(&summary));
                failed |= summary.stopped_early.is_some();
            }
            Err(e) => {
                error!("{}: triage failed: {}", email, e);
                failed = true;
            }
        }
    }
    Ok(failed)
}

/// The decision model when TRIAGE_DECISION_URL and _MODEL are set, else the
/// chat model. With TRIAGE_MODEL_URL and _LARGE also set, the decision
/// model's answers below the apply bar get a second pass from the large model.
fn build_classifier(config: &TriageConfig) -> Result<ConfiguredClassifier, String> {
    if let Some((url, model)) = config.decision_settings() {
        let backend = LocalDecisionServer::new(url, config.model_timeout)?;
        let second = match config.second_pass_settings() {
            Some((large_url, large_model)) => Some(SecondPass::new(
                LocalModelServer::new(large_url, config.model_timeout)?,
                large_model.to_string(),
                config.min_confidence,
            )),
            None => None,
        };
        return Ok(ConfiguredClassifier::Decision(
            DecisionClassifier::new(backend, model.to_string()),
            second,
        ));
    }
    let (model_url, small_model) = config.model_settings()?;
    let backend = LocalModelServer::new(model_url, config.model_timeout)?;
    Ok(ConfiguredClassifier::Chat(EscalatingClassifier::new(
        backend,
        small_model.to_string(),
        config.large_model.clone(),
        config.escalate_below,
    )))
}

/// Answered mail leaves the Needs Reply folder for the INBOX. A failure is
/// reported and does not fail the apply: the moves it made stand.
async fn release_answered_mail(
    store: &TriageStore,
    client: &rustymail::triage::runner::Client,
    email: &str,
    inbox: &str,
    folders: &FolderMap,
) {
    let released = match KnownPeople::load(store.pool(), email).await {
        Ok(known) => release_answered(client, email, folders, &known, inbox).await,
        Err(e) => Err(e.into()),
    };
    match released {
        Ok(0) => {}
        Ok(count) => println!(
            "{}: {} answered message(s) back from Needs Reply",
            email, count
        ),
        Err(e) => error!(
            "{}: releasing answered Needs Reply mail failed: {}",
            email, e
        ),
    }
}

fn describe_run(s: &AccountSummary) -> String {
    format!(
        "{}: examined {}, by rule {}, by model {}, unparseable {}, watermark {}{}",
        s.account,
        s.examined,
        s.by_rule,
        s.by_model,
        s.unparseable,
        s.watermark
            .map_or_else(|| "none".to_string(), |uid| format!("UID {}", uid)),
        s.stopped_early
            .as_deref()
            .map(|e| format!(", stopped early: {}", e))
            .unwrap_or_default()
    )
}

/// Returns whether any account had failures.
async fn apply_all(
    pool: &SqlitePool,
    store: &TriageStore,
    accounts: &[String],
    cli: &Cli,
    config: &TriageConfig,
) -> Result<bool, Box<dyn std::error::Error>> {
    let folders = match &config.folder_map_file {
        Some(path) => FolderMap::load(path)?,
        None => FolderMap::default(),
    };
    let options = ApplyOptions {
        min_confidence: cli.min_confidence.unwrap_or(config.min_confidence),
        buckets: config.apply_buckets.clone(),
        limit: cli.limit,
    };
    let mut failed = false;
    for email in accounts {
        let Some(account) = active_account(pool, email).await else {
            error!("{}: not an active account", email);
            failed = true;
            continue;
        };
        let client = match account.connect().await {
            Ok(client) => client,
            Err(e) => {
                error!("{}: cannot connect: {}", email, e);
                failed = true;
                continue;
            }
        };
        let result = apply_account(store, &client, email, &config.folder, &folders, &options).await;
        release_answered_mail(store, &client, email, &config.folder, &folders).await;
        if let Err(e) = client.logout().await {
            error!("{}: logout failed: {}", email, e);
        }
        match result {
            Ok(summary) => {
                print!("{}", format_apply_summary(&summary));
                failed |= !summary.failures.is_empty();
            }
            Err(e) => {
                error!("{}: apply failed: {}", email, e);
                failed = true;
            }
        }
    }
    Ok(failed)
}
