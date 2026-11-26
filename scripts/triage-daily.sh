#!/bin/bash
# Daily triage: propose moves for TRIAGE_ACCOUNTS (INBOX and spam folder), save the report,
# and post a macOS notification saying how many messages wait for review. Never moves mail:
# moves happen only with `rustymail-triage --apply` after the owner's review.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"
# A setting from .env, or empty when it is not there.
env_value() { { grep -E "^$1=" .env 2>/dev/null || true; } | tail -1 | cut -d= -f2-; }
BIN="${TRIAGE_DAILY_BIN:-$(env_value TRIAGE_DAILY_BIN)}"
BIN="${BIN:-$REPO/target/release/rustymail-triage}"
REPORT_DIR="${TRIAGE_REPORT_DIR:-$(env_value TRIAGE_REPORT_DIR)}"
REPORT_DIR="${REPORT_DIR:-$REPO/logs/triage}"
DB_URL="${CACHE_DATABASE_URL:-$(env_value CACHE_DATABASE_URL)}"
DB="${DB_URL#sqlite:}"
DB="${DB%%\?*}"
mkdir -p "$REPORT_DIR"
REPORT="$REPORT_DIR/triage-$(date +%Y-%m-%d).txt"

notify() { osascript -e "display notification \"$1\" with title \"RustyMail triage\"" || true; }

# Proposals this run makes are the ones to announce (older pending ones were announced before).
STARTED=$(sqlite3 "$DB" "SELECT datetime('now')")
if ! nice -n 10 "$BIN" > "$REPORT" 2> "$REPORT.log"; then
    notify "Triage run failed; see $REPORT.log"
    exit 1
fi
# New proposals a review would act on: INBOX mail to file, and spam-folder mail to rescue.
NEW="applied_at IS NULL AND created_at >= '$STARTED'"
WAITING=$(sqlite3 "$DB" ".timeout 60000" "SELECT count(*) FROM triage_proposals WHERE $NEW
  AND ((folder = 'INBOX' AND coalesce(correction, bucket) <> 'Keep') OR (folder <> 'INBOX' AND coalesce(correction, bucket) <> 'Spam'))")
RESCUES=$(sqlite3 "$DB" ".timeout 60000" "SELECT count(*) FROM triage_proposals WHERE $NEW
  AND folder <> 'INBOX' AND coalesce(correction, bucket) <> 'Spam'")
if [ "$WAITING" -gt 0 ]; then
    notify "$WAITING new messages to review ($RESCUES to rescue from spam). Report: $REPORT"
fi
