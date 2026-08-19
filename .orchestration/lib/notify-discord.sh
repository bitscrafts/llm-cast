#!/usr/bin/env bash
# =============================================================================
# notify-discord.sh — post a run lifecycle message to a Discord channel.
#
# Sends a message to DISCORD_CHANNEL_ID using the bot token in DISCORD_BOT_TOKEN
# (both read from the environment, which the harness loads from ~/.hermes/.env).
# Fires at spec START and END so the operator can follow long autonomous runs
# from Discord without watching the terminal.
#
# Usage:
#   notify-discord.sh "spec started" "specs/01-x.md" "/projects/repo" [details]
#   notify-discord.sh "spec complete ✓" "specs/01-x.md" "/projects/repo" "8/8 exit criteria, gate green"
#
# Silent failure: if Discord env is missing or the POST fails, exit 0 anyway —
# a notification must never block or fail a run.
# =============================================================================
set -uo pipefail

EVENT="${1:-event}"
SPEC="${2:-}"
ROOT="${3:-}"
DETAILS="${4:-}"

TOKEN="${DISCORD_BOT_TOKEN:-}"
CHANNEL="${DISCORD_CHANNEL_ID:-}"
# The env loader keeps quotes from `export KEY="value"` lines — strip them.
TOKEN="${TOKEN%\"}"; TOKEN="${TOKEN#\"}"
CHANNEL="${CHANNEL%\"}"; CHANNEL="${CHANNEL#\"}"
[ -n "$TOKEN" ] && [ -n "$CHANNEL" ] || exit 0   # no Discord configured — silent

SPEC_NAME="$(basename "$SPEC" 2>/dev/null || echo "$SPEC")"
HOST_NAME="$(hostname 2>/dev/null || echo 'unknown')"

MSG="**[pi-orchestration] $EVENT**"
[ -n "$SPEC_NAME" ] && MSG="$MSG
📄 spec: \`$SPEC_NAME\`"
[ -n "$ROOT" ] && MSG="$MSG
📁 repo: \`$ROOT\`"
[ -n "$DETAILS" ] && MSG="$MSG
ℹ️ $DETAILS"
MSG="$MSG
🖥 host: $HOST_NAME"

# No escaping: message content is controlled (spec names, repo paths, our own
# markdown markers). Backticks in paths render as code blocks on Discord.
TMP="$(mktemp)"
printf '{"content": %s}' "$(printf '%s' "$MSG" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))' 2>/dev/null || printf '"%s"' "$MSG")" > "$TMP"

curl -s -o /dev/null -w "%{http_code}" -X POST \
    "https://discord.com/api/v10/channels/${CHANNEL}/messages" \
    -H "Authorization: Bot ${TOKEN}" \
    -H "Content-Type: application/json" \
    --data @"$TMP" >/dev/null 2>&1

rm -f "$TMP"
exit 0   # never fail the run on a notification
