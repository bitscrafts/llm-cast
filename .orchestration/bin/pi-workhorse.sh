#!/usr/bin/env bash
#
# pi-workhorse.sh — drive `pi` as the implementer while Claude Code orchestrates.
#
# Part of the pi-orchestration bundle. SELF-CONTAINED: every file it needs --
# DIRECTIVES.md, config.env, lib/quality-gate.sh, lib/validate-exit-criteria.sh
# -- ships inside the bundle. It reads nothing from the host's CLAUDE.md or
# AGENTS.md, and requires nothing of the target project. Those carry solution detail and may be absent or written for a
# different audience; operating rules travel with the harness.
#
# Roles are separated by TOOL GRANT, not by instruction:
#   implement  read,write,edit,bash,grep,find,ls   — can change the tree
#   repair     read,write,edit,bash,grep,find,ls   — continues the same session
#   review     read,grep,find,ls                   — structurally CANNOT edit
#   gate       (no model)                          — the objective check
#   validate   (no model)                          — exit-criteria check
#   run        (the main entrance)                 — drives phases 2-6 end to end
#   handoff    (no model)                          — checks HANDOFF.md was updated
#   doctor     (no model)                          — verifies every cross-reference resolves
#
# A reviewer that cannot write is worth more than a reviewer told not to.
#
# Usage:
#   pi-workhorse.sh run       <spec> [root]         <-- START HERE
#   pi-workhorse.sh implement <spec> [root] [model]
#   pi-workhorse.sh repair    <spec> [root] [model]     # --escalate for the big model
#   pi-workhorse.sh review    <spec> [root] [model]
#   pi-workhorse.sh gate      [root]
#   pi-workhorse.sh validate  <spec> [root]
#   pi-workhorse.sh handoff   [root]
#   pi-workhorse.sh doctor    [claude_skills_dir] [pi_skills_dir]
#
# Model: none is hardcoded. Omitting it uses pi's own configuration. Pass
# `--escalate` as the model argument to use config.env's ORCH_ESCALATION_MODEL.
#
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORCH_HOME_DEFAULT="$(dirname "$HERE")"

# Resolve the bundle. Precedence, most specific first:
#   1. $ORCH_HOME if already set
#   2. <project>/.orchestration  -- a bundle vendored into the repo
#   3. the bundle this script was invoked from
# A project-local install lets a repo pin its own copy, so a checkout carries
# its rules with it and two projects can run different versions.
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

PI="${PI_BIN:-$(command -v pi || echo /root/.local/bin/pi)}"
DIRECTIVES="$ORCH_HOME/pi/DIRECTIVES.md"

CMD="${1:-}"
[ -z "$CMD" ] && { sed -n '20,29p' "$0" >&2; exit 2; }
shift

die() { echo "pi-workhorse: $*" >&2; exit 2; }
[ -x "$PI" ] || die "pi binary not found (set PI_BIN)"

# ------------------------------------------------------------------ helpers
session_dir_for() {   # one session per spec, so repair keeps context
    local root="$1"
    local spec="$2"
    local d="$root/_tmp/pi-sessions/$(basename "$spec" .md)"
    mkdir -p "$d"; echo "$d"
}

# The gate ships WITH the bundle. It detects the project type itself and falls
# back to a project-provided deploy/scripts/quality-gate.sh only as an override.
# Reaching into the target project for a required file is what made the earlier
# version not self-contained.
run_gate() {
    local root="${1:-.}"
    [ -x "$ORCH_HOME/lib/quality-gate.sh" ] || die "bundle incomplete: lib/quality-gate.sh missing"
    bash "$ORCH_HOME/lib/quality-gate.sh" "$root"
}

# Also ships with the bundle. No host or project dependency.
run_validate() {
    local spec="$1"
    local root="${2:-.}"
    [ -x "$ORCH_HOME/lib/validate-exit-criteria.sh" ] || die "bundle incomplete: lib/validate-exit-criteria.sh missing"
    bash "$ORCH_HOME/lib/validate-exit-criteria.sh" "$spec" "$root"
}

# HANDOFF.md is the implementation diary. Enforced, not suggested: a task is
# not complete until it records what was done and what comes next.
check_handoff() {
    local root="${1:-.}"
    local h="$root/HANDOFF.md"
    [ -f "$h" ] || { echo "HANDOFF MISSING: $h does not exist" >&2; return 1; }
    if [ -n "$(find "$h" -mmin +240 2>/dev/null)" ]; then
        echo "HANDOFF STALE: $h not modified in the last 4 hours" >&2; return 1
    fi
    echo "HANDOFF OK: $h updated recently"
}

# -------------------------------------------------------------------- roles
case "$CMD" in
  implement|repair|review)
    SPEC="${1:-}"; [ -n "$SPEC" ] || die "spec path required"
    ROOT="${2:-$(cd "$(dirname "$SPEC")/.." && pwd)}"
    MODEL="${3:-${ORCH_MODEL:-}}"
    [ -f "$SPEC" ] || die "no such spec: $SPEC"
    [ -d "$ROOT" ] || die "no such project root: $ROOT"
    [ -f "$DIRECTIVES" ] || die "DIRECTIVES.md missing at $DIRECTIVES — bundle is incomplete"

    # A model from a non-default provider needs the provider too. ORCH_PROVIDER
    # pairs with an explicitly passed model, so any catalogue entry works:
    #   ORCH_PROVIDER=ollama-cloud pi-workhorse.sh review <spec> <root> kimi-k2.7-code:cloud
    PROVIDER="${ORCH_PROVIDER:-}"
    if [ "$MODEL" = "--escalate" ]; then
        MODEL="$ORCH_ESCALATION_MODEL"; PROVIDER="$ORCH_ESCALATION_PROVIDER"
        echo "pi-workhorse: escalating to $MODEL (${PROVIDER:-default provider})" >&2
    fi

    SD="$(session_dir_for "$ROOT" "$SPEC")"
    SPEC_REL="${SPEC#$ROOT/}"

    case "$CMD" in
      implement)
        TOOLS="read,write,edit,bash,grep,find,ls"; CONT=""
        TASK="Implement the spec at $SPEC_REL, following it exactly.

Write the tests from its TDD Contract FIRST, then the production code.
Verify with:  $ORCH_HOME/lib/quality-gate.sh $ROOT
Do not stop until it exits 0, or until you hit something the spec gets wrong.

Then update HANDOFF.md: what you did, the outcome, and what comes next.

If you stop because the SPEC is wrong rather than the code, make the FIRST LINE
of your reply exactly:  SPEC-DEFECT: <one-line summary>
That marker is how the orchestrator knows to stop the loop and read, instead of
spending a review pass on an unchanged tree.

Reply in at most 25 lines: files changed, the gate's stage results, every
'test result:' line verbatim, and anything in the spec you found wrong."
        ;;
      repair)
        TOOLS="read,write,edit,bash,grep,find,ls"; CONT="-c"
        TASK="${ORCH_REPAIR_MSG:-The gate is still failing. Read the failure, fix the cause rather than the symptom, and re-run the gate. If the spec is what is wrong, say so instead of working around it. Reply in at most 15 lines.}"
        ;;
      review)
        # Read-only by tool grant: it cannot edit even if it decides to.
        TOOLS="read,grep,find,ls"; CONT=""
        TASK="Review the working-tree changes against the spec at $SPEC_REL.
You have READ-ONLY tools by design. Do not attempt to modify anything.

Read the load-bearing code, not just the diff stat: the parts where the spec's
judgement lives — error paths, ordering, persistence, wire formats, anything the
spec called out as a key decision. Check the ARCHITECTURE the spec describes is
actually what was built, not merely that tests pass.

Specifically: does every Requirement have a corresponding change; was any
existing test weakened, skipped or deleted; does each new check assert on what
the CONSUMER received rather than on what the producing function returned; any
hardcoded absolute paths; any regenerated fixture.

Reply in at most 20 lines: PASS or FAIL on line 1, then findings worst-first.
FAIL if any requirement is unimplemented or any test was weakened."
        ;;
    esac

    cd "$ROOT" || die "cannot cd to $ROOT"

    ARGS=(-p)
    [ -n "$CONT" ] && ARGS+=("$CONT")
    ARGS+=(--session-dir "$SD" --tools "$TOOLS")
    ARGS+=(--max-tool-iterations "${ORCH_MAX_TOOL_ITERATIONS:-60}")
    # --request-timeout bounds a single provider API call. It does NOT bound the
    # agent loop, so a wall-clock cap is applied separately below. Both were
    # documented in config.env and neither was wired: a phase ran 40 minutes
    # with nothing able to stop it.
    ARGS+=(--request-timeout "${ORCH_REQUEST_TIMEOUT:-600}")
    [ -n "$MODEL" ]    && ARGS+=(--model "$MODEL")
    [ -n "$PROVIDER" ] && ARGS+=(--provider "$PROVIDER")
    # pi auto-discovers only its user-wide skills dir. A project-local bundle
    # points at its own with --skill, which accepts a directory and repeats.
    for sk in "$ORCH_HOME"/pi/skills/*/; do
        [ -f "$sk/SKILL.md" ] && ARGS+=(--skill "$sk")
    done
    ARGS+=(--append-system-prompt "$(cat "$DIRECTIVES")")

    # Announce the model BEFORE the call: on a long turn this is the only
    # signal about what is actually running, and an escalation that silently
    # did not take effect is worth seeing immediately.
    printf '  [%s] -> %s%s · tools=%s\n' "$CMD" \
        "${MODEL:-<pi default>}" "${PROVIDER:+ ($PROVIDER)}" "$TOOLS" >&2

    # Not exec: we need to report usage after pi returns.
    # Wall-clock cap on the whole phase. --request-timeout only bounds one API
    # call; a tool loop can spin well past it. timeout returns 124 on expiry.
    PT="${ORCH_PHASE_TIMEOUT:-1800}"
    if command -v timeout >/dev/null 2>&1 && [ "$PT" -gt 0 ] 2>/dev/null; then
        timeout --signal=TERM --kill-after=30 "$PT" "$PI" "${ARGS[@]}" "$TASK"
        rc=$?
        [ "$rc" -eq 124 ] && echo "  [$CMD] TIMED OUT after ${PT}s (ORCH_PHASE_TIMEOUT)" >&2
    else
        "$PI" "${ARGS[@]}" "$TASK"
        rc=$?
    fi
    [ -x "$ORCH_HOME/lib/usage.sh" ] && bash "$ORCH_HOME/lib/usage.sh" "$SD" "$CMD" >&2
    exit $rc
    ;;

  # ---------------------------------------------------------------- run
  # The main entrance. Phases 2-6 are a state machine, not a judgement:
  # implement, gate, repair up to N times, escalate, review, validate. Driving
  # that from a Claude subagent would spend Claude tokens re-reading gate output
  # and pi's replies -- exactly the cost this bundle exists to remove. So it is
  # a shell loop, and it STOPS and hands back the moment anything needs
  # judgement. Exit codes: 3 gate still failing after escalation, 4 exit criteria
  # unmet, 5 the workhorse reports the spec itself is wrong, 6 implement
  # produced nothing even after escalation.
  # Phase 7 (read the diff, commit) is never run here; it belongs to the
  # orchestrator.
  run)
    SPEC="${1:-}"; [ -n "$SPEC" ] || die "spec path required"
    ROOT="${2:-$(cd "$(dirname "$SPEC")/.." && pwd)}"
    SELF="$HERE/pi-workhorse.sh"
    rounds="${ORCH_MAX_REPAIR_ROUNDS:-2}"

    echo "== phase 2: implement =="
    imp_log="$ROOT/_tmp/pi-implement.$$.log"
    mkdir -p "$(dirname "$imp_log")"
    before="$(cd "$ROOT" && git status --porcelain 2>/dev/null | md5sum)"
    "$SELF" implement "$SPEC" "$ROOT" 2>&1 | tee "$imp_log"
    imp_rc="${PIPESTATUS[0]}"
    after="$(cd "$ROOT" && git status --porcelain 2>/dev/null | md5sum)"

    # Escalate when implement PRODUCES NOTHING, not only when the gate fails.
    # The repair ladder below never engages if phase 2 stalls: a flash model
    # once ran 40 minutes on a large repo, wrote nothing, and never reached the
    # gate, so escalation had to be done by hand. A timeout (124) or an
    # unchanged tree is the signal.
    if [ "$imp_rc" -eq 124 ] || { [ "$before" = "$after" ] && ! grep -qE '^\s*SPEC-DEFECT:' "$imp_log"; }; then
        if [ "$imp_rc" -eq 124 ]; then
            echo "== phase 2: implement timed out -- escalating =="
        else
            echo "== phase 2: implement changed nothing -- escalating =="
        fi
        "$SELF" implement "$SPEC" "$ROOT" --escalate 2>&1 | tee "$imp_log"
        after="$(cd "$ROOT" && git status --porcelain 2>/dev/null | md5sum)"
        if [ "$before" = "$after" ] && ! grep -qE '^\s*SPEC-DEFECT:' "$imp_log"; then
            echo
            echo "STOPPED: implement produced no change even after escalation."
            echo "The spec is probably too large for a workhorse on this repo. Narrow it."
            rm -f "$imp_log"
            exit 6
        fi
    fi

    # A reported spec defect ends the loop here. Continuing would review an
    # unchanged tree and validate criteria for a feature nobody built -- work
    # that costs tokens and tells you nothing you were not just told.
    if grep -qE '^\s*SPEC-DEFECT:' "$imp_log"; then
        echo
        grep -m1 -E '^\s*SPEC-DEFECT:' "$imp_log"
        echo "STOPPED: the workhorse reports the SPEC is wrong, and did not code around it."
        echo "Verify the claim against the code yourself, then amend the spec. Do not let pi amend it."
        rm -f "$imp_log"
        exit 5
    fi
    rm -f "$imp_log"

    n=0
    while : ; do
        echo "== phase 3: gate =="
        if "$SELF" gate "$ROOT"; then break; fi
        n=$((n+1))
        if [ "$n" -gt "$rounds" ]; then
            echo "== phase 4: gate still failing after $rounds rounds -- escalating =="
            "$SELF" repair "$SPEC" "$ROOT" --escalate || true
            if "$SELF" gate "$ROOT"; then break; fi
            echo
            echo "STOPPED: gate fails after escalation. This needs judgement --"
            echo "read the failure and decide whether the spec or the code is wrong."
            exit 3
        fi
        echo "== phase 4: repair (round $n/$rounds) =="
        "$SELF" repair "$SPEC" "$ROOT" || true
    done

    echo "== phase 5: review (read-only) =="
    "$SELF" review "$SPEC" "$ROOT" || true

    echo "== phase 6: validate exit criteria =="
    "$SELF" validate "$SPEC" "$ROOT" || {
        echo
        echo "STOPPED: exit criteria not satisfied. Read them before committing."
        exit 4
    }

    echo
    echo "== task total =="
    [ -x "$ORCH_HOME/lib/usage.sh" ] && bash "$ORCH_HOME/lib/usage.sh" \
        "$ROOT/_tmp/pi-sessions/$(basename "$SPEC" .md)" "TOTAL"
    echo
    echo "PHASES 2-6 COMPLETE. Phase 7 is yours:"
    echo "  read the load-bearing diff, confirm the architecture, then commit."
    echo "  git -C $ROOT diff --stat"
    ;;

  gate)     run_gate "${1:-.}" ;;
  doctor)   bash "$ORCH_HOME/lib/doctor.sh" "$ORCH_HOME" "${1:-$HOME/.claude/skills}" "${2:-$HOME/.pi/agent/skills}" ;;
  validate) [ -n "${1:-}" ] || die "spec path required"; run_validate "$1" "${2:-.}" ;;
  handoff)  check_handoff "${1:-.}" ;;
  *) die "unknown command: $CMD" ;;
esac
