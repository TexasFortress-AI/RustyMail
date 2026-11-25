#!/bin/bash
# Install (or reinstall) the daily triage run as a launchd agent for this user.
# Time from TRIAGE_DAILY_HOUR / TRIAGE_DAILY_MINUTE in .env. Remove with --uninstall.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
# A setting from .env, or empty when it is not there.
env_value() { { grep -E "^$1=" "$REPO/.env" 2>/dev/null || true; } | tail -1 | cut -d= -f2-; }
LABEL="${TRIAGE_DAILY_LABEL:-$(env_value TRIAGE_DAILY_LABEL)}"
LABEL="${LABEL:-ai.texasfortress.rustymail.triage-daily}"
HOUR="${TRIAGE_DAILY_HOUR:-$(env_value TRIAGE_DAILY_HOUR)}"
MINUTE="${TRIAGE_DAILY_MINUTE:-$(env_value TRIAGE_DAILY_MINUTE)}"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$PLIST"
    echo "removed $LABEL"
    exit 0
fi
: "${HOUR:?set TRIAGE_DAILY_HOUR in .env}" "${MINUTE:?set TRIAGE_DAILY_MINUTE in .env}"
mkdir -p "$REPO/logs" "$(dirname "$PLIST")"
sed -e "s|__LABEL__|$LABEL|g" -e "s|__REPO__|$REPO|g" -e "s|__HOUR__|$HOUR|g" -e "s|__MINUTE__|$MINUTE|g" \
    "$REPO/scripts/launchd/rustymail-triage-daily.plist.template" > "$PLIST"
plutil -lint "$PLIST" >/dev/null
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "installed $LABEL: daily at $HOUR:$(printf %02d "$MINUTE")"
