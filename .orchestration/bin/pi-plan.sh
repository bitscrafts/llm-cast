#!/usr/bin/env bash
#
# pi-plan.sh — drive `pi` as the PLANNER. Turns a feature brief (or thin draft
# spec) into implementable, numbered spec parts under specs/.
#
# Part of the pi-orchestration bundle. SELF-CONTAINED: every file it needs —
# DIRECTIVES-planner.md, config.env — ships inside the bundle. It reads
# nothing from the host's CLAUDE.md or AGENTS.md, and requires nothing of the
# target project except a specs/ directory to write into.
#
# The workhorse (pi-workhorse.sh) implements; the planner specifies. Same
# no-commit law, opposite deliverable: the planner's product IS the spec.
#
# Usage:
#   pi-plan.sh plan <brief|draft-spec> [root] [model] [provider]   <-- START HERE
#   pi-plan.sh doctor  [claude_skills_dir] [pi_skills_dir]
#
# Model: defaults to ORCH_PLANNER_MODEL (config.env), which defaults to the
# escalation/planner tier z-ai/glm-5.2 (provider nvidia). Pass a model and
# provider positionally to override for one run, e.g.:
#   pi-plan.sh plan briefs/02-hls.md . deepseek-v4-pro deepseek
#
# Exit codes (consistent with pi-workhorse.sh):
#   6  the planner produced no spec files
#   7  the planner COMMITTED (HEAD moved) — DIRECTIVES forbids any git write
#   2  usage / environment error
#
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORCH_HOME_DEFAULT="$(dirname "$HERE")"

# Resolve the bundle. Precedence, most specific first:
#   1. $ORCH_HOME if already set
#   2. <project>/.orchestration  -- a bundle vendored into the repo
#   3. the bundle this script was invoked from
resolve_orch_home() {
    [ -n "${ORCH_HOME:-}" ] && { echo "$ORCH_HOME"; return; }
    local d="${ORCH_PROJECT:-$PWD}"
    while [ "$d" != "/" ] && [ -n "$d" ]; do
        [ -f "$d/.orchestration/config.env" ] && { echo "$d/.orchestration"; return; }
        d="$(dirname "$d")"
    done
    echo "$ORCH_HOME_DEFAULT"
}
ORCH_HOME="$(resolve_orch_home)"
[ -f "$ORCH_HOME/config.env" ] && . "$ORCH_HOME/config.env"

PI="${PI_BIN:-$(command -v pi 2>/dev/null || true)}"
[ -n "$PI" ] || { echo "pi-plan: pi not found on PATH and PI_BIN is unset — install pi or set PI_BIN" >&2; exit 2; }
[ -x "$PI" ] || { echo "pi-plan: pi binary not found (set PI_BIN)" >&2; exit 2; }

DIRECTIVES="$ORCH_HOME/pi/DIRECTIVES-planner.md"
[ -f "$DIRECTIVES" ] || { echo "pi-plan: DIRECTIVES-planner.md missing at $DIRECTIVES — bundle is incomplete" >&2; exit 2; }

CMD="${1:-}"
[ -z "$CMD" ] && { sed -n '17,24p' "$0" >&2; exit 2; }
shift

die() { echo "pi-plan: $*" >&2; exit 2; }

# One session per brief, so a re-plan continues context.
session_dir_for() {
    local root="$1" brief="$2"
    local d="$root/_tmp/pi-sessions/planner-$(basename "$brief" .md)"
    mkdir -p "$d"; echo "$d"
}

case "$CMD" in
  plan)
    BRIEF="${1:-}"; [ -n "$BRIEF" ] || die "brief or draft spec path required"
    ROOT="${2:-$(cd "$(dirname "$BRIEF")/.." && pwd)}"
    MODEL="${3:-${ORCH_PLANNER_MODEL:-}}"
    PROVIDER="${4:-${ORCH_PLANNER_PROVIDER:-}}"
    [ -f "$BRIEF" ] || die "no such brief: $BRIEF"
    [ -d "$ROOT" ] || die "no such project root: $ROOT"
    [ -d "$ROOT/specs" ] || die "no specs/ directory in $ROOT — create it first"

    SD="$(session_dir_for "$ROOT" "$BRIEF")"
    BRIEF_REL="${BRIEF#$ROOT/}"
    head0="$(cd "$ROOT" && git rev-parse HEAD 2>/dev/null)"

    # Same mechanical no-commit law as the workhorse: DIRECTIVES-planner
    # section 1 forbids it at prompt level; the harness detects it regardless.
    check_no_commit() {
        local now
        now="$(cd "$ROOT" && git rev-parse HEAD 2>/dev/null)"
        if [ -n "$head0" ] && [ "$now" != "$head0" ]; then
            echo
            echo "VIOLATION: the planner COMMITTED (HEAD moved $head0 -> $now)."
            echo "DIRECTIVES-planner section 1 forbids any git write. STOPPING."
            exit 7
        fi
    }

    # The planner's deliverable is spec files. HANDOFF/_tmp edits do not count.
    specs_changed() {
        local status
        status="$(cd "$ROOT" && git status --porcelain 2>/dev/null)"
        [ -n "$(printf '%s' "$status" | grep -E 'specs/' | head -1)" ]
    }

    cd "$ROOT" || die "cannot cd to $ROOT"

    # Isolate this run's usage numbers from earlier runs of the same brief.
    bash "$ORCH_HOME/lib/usage.sh" "$SD" "" --clear >/dev/null 2>&1 || true

    ARGS=(-p --session-dir "$SD")
    ARGS+=(--tools "read,write,edit,grep,find,ls")
    ARGS+=(--max-tool-iterations "${ORCH_MAX_TOOL_ITERATIONS:-60}")
    ARGS+=(--request-timeout "${ORCH_REQUEST_TIMEOUT:-600}")
    [ -n "$MODEL" ]    && ARGS+=(--model "$MODEL")
    [ -n "$PROVIDER" ] && ARGS+=(--provider "$PROVIDER")
    [ -f "$ORCH_HOME/pi/skills/planner/SKILL.md" ] \
        && ARGS+=(--skill "$ORCH_HOME/pi/skills/planner")
    ARGS+=(--append-system-prompt "$(cat "$DIRECTIVES")")

    TASK="Plan the feature described in $BRIEF_REL for this project.

Follow the planner skill: read the project's AGENTS.md/CLAUDE.md and existing
specs/ first, then write the feature as one or more numbered spec files under
specs/ (split into parts when a part would exceed ~7 exit criteria). Each part
is self-contained: Overview, Requirements, Architecture, TDD Contract, Exit
Criteria as checkboxes with exact shell commands, Guardrails.

You are the planner, not the implementer. Your deliverable is the spec
file(s). Do not create or edit any implementation files.

Reply in the planner format: first line the number of spec files written, then
one line per file, then any open questions for the orchestrator. At most 15
lines."

    printf '  [plan] -> %s%s · skills=planner\n' \
        "${MODEL:-<pi default>}" "${PROVIDER:+ ($PROVIDER)}" >&2

    PT="${ORCH_PHASE_TIMEOUT:-1800}"
    if command -v timeout >/dev/null 2>&1 && [ "$PT" -gt 0 ] 2>/dev/null; then
        timeout --signal=TERM --kill-after=30 "$PT" "$PI" "${ARGS[@]}" "$TASK"
        rc=$?
        [ "$rc" -eq 124 ] && echo "  [plan] TIMED OUT after ${PT}s (ORCH_PHASE_TIMEOUT)" >&2
    else
        "$PI" "${ARGS[@]}" "$TASK"
        rc=$?
    fi
    check_no_commit
    [ -x "$ORCH_HOME/lib/usage.sh" ] && bash "$ORCH_HOME/lib/usage.sh" "$SD" "plan" >&2

    if [ "$rc" -ne 0 ]; then
        echo "pi-plan: planner exited $rc without producing specs" >&2
        exit "$rc"
    fi
    if ! specs_changed; then
        echo
        echo "PLANNER PRODUCED NOTHING: no changes under specs/ (HANDOFF/_tmp edits"
        echo "do not count). The orchestrator must re-brief or escalate the plan."
        exit 6
    fi

    echo "pi-plan: done. Specs written under $ROOT/specs/ (uncommitted)."
    exit 0
    ;;

  doctor)
    bash "$ORCH_HOME/lib/doctor.sh" "$ORCH_HOME" "${1:-$HOME/.claude/skills}" "${2:-$HOME/.pi/agent/skills}"
    ;;

  *)
    sed -n '17,24p' "$0" >&2
    exit 2
    ;;
esac
