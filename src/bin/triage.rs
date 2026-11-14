// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! rustymail-triage: propose a bucket for new INBOX mail (phase 1, propose-only).
//!
//! Nothing is moved. Proposals go to the triage_proposals table; review them
//! with `rustymail-triage --report`.

use clap::Parser;
use log::{error, info};
use rustymail::imap::account_connect::load_active_accounts;
use rustymail::triage::config::TriageConfig;
use rustymail::triage::model::{EscalatingClassifier, LocalModelServer};
use rustymail::triage::report::format_report;
use rustymail::triage::rules::SenderRules;
use rustymail::triage::runner::{RunOptions, TriageRunner};
use rustymail::triage::store::TriageStore;
use sqlx::SqlitePool;

#[derive(Parser)]
#[command(
    name = "rustymail-triage",
    about = "Propose triage buckets for new mail (moves nothing)"
)]
struct Cli {
    #[arg(
        long,
        env = "CACHE_DATABASE_URL",
        default_value = "sqlite:data/email_cache.db"
    )]
    database_url: String,

    /// Account(s) to triage or report on; defaults to TRIAGE_ACCOUNTS.
    #[arg(long)]
    account: Vec<String>,

    /// Also triage messages received in the last N days (backfill).
    #[arg(long)]
    since_days: Option<u32>,

    /// Triage at most N messages per account (oldest first).
    #[arg(long)]
    limit: Option<usize>,

    /// Classify and log only: write no proposals, leave watermarks alone.
    #[arg(long)]
    dry_run: bool,

    /// Print pending proposals with dashboard links instead of triaging.
    #[arg(long)]
    report: bool,

    /// Maximum proposals in the report.
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

    if cli.report {
        let filter = (accounts.len() == 1).then(|| accounts[0].as_str());
        let proposals = store.pending_proposals(filter, cli.report_limit).await?;
        print!("{}", format_report(&proposals, &config.dashboard_url));
        return Ok(());
    }

    if accounts.is_empty() {
        return Err("no accounts: pass --account or set TRIAGE_ACCOUNTS".into());
    }
    let (model_url, small_model) = config.model_settings()?;
    let backend = LocalModelServer::new(model_url, config.model_timeout)?;
    let senders = match &config.sender_rules_file {
        Some(path) => SenderRules::load(path)?,
        None => SenderRules::default(),
    };
    let runner = TriageRunner {
        store,
        classifier: EscalatingClassifier::new(
            backend,
            small_model.to_string(),
            config.large_model.clone(),
            config.escalate_below,
        ),
        senders,
        folder: config.folder.clone(),
        fetch_batch: config.fetch_batch,
        excerpt_chars: config.excerpt_chars,
    };
    let options = RunOptions {
        since_days: cli.since_days,
        limit: cli.limit,
        dry_run: cli.dry_run,
    };

    let mut failed = false;
    for email in &accounts {
        let Some(account) = load_active_accounts(&pool, Some(email))
            .await?
            .into_iter()
            .next()
        else {
            error!("{}: not an active account", email);
            failed = true;
            continue;
        };
        match runner.run_account(&account, options).await {
            Ok(s) => {
                info!(
                    "{}: examined {}, rule {}, model {}, unparseable {}{}{}",
                    s.account,
                    s.examined,
                    s.by_rule,
                    s.by_model,
                    s.unparseable,
                    if s.initialized {
                        ", watermark initialized"
                    } else {
                        ""
                    },
                    s.stopped_early
                        .as_deref()
                        .map(|e| format!(", stopped early: {}", e))
                        .unwrap_or_default()
                );
                failed |= s.stopped_early.is_some();
            }
            Err(e) => {
                error!("{}: triage failed: {}", email, e);
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
    Ok(())
}
