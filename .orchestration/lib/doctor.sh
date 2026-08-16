#!/usr/bin/env bash
#
# doctor.sh — verify every cross-reference in the bundle actually resolves.
#
#   doctor.sh [orch_home] [claude_skills_dir] [pi_skills_dir]
#
# Skills name the skills and scripts they hand off to, so the next step is
# deterministic rather than a judgement the model makes at runtime. That is only
# an improvement if the names are real: a skill naming a renamed or uninstalled
# skill fails confidently, which is worse than failing loudly.
#
# This checks: every `skill: X` reference resolves to an installed skill, every
# scripts/ helper a skill names exists and is executable, and every lib/ script
# the harness calls is present.
#
# LIMIT, stated because a partial check that reads as complete is the defect
# this bundle exists to prevent: doctor only sees references written in the
# recognised forms -- skill names in backticks after the word "skill", and paths
# matching scripts/*.sh. A handoff described only in prose is invisible to it.
# If you add a cross-reference, write it in one of those forms or doctor will
# report OK while the reference is broken.
#
set -uo pipefail
ORCH="${1:-${ORCH_HOME:-$HOME/.pi-orchestration}}"
CSK="${2:-$HOME/.claude/skills}"
PSK="${3:-$HOME/.pi/agent/skills}"
bad=0
ok()   { printf '  OK    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; bad=1; }

echo "pi-orchestration doctor"
echo "  orch_home=$ORCH"

for f in bin/pi-workhorse.sh lib/quality-gate.sh lib/validate-exit-criteria.sh \
         lib/doctor.sh pi/DIRECTIVES.md config.env; do
    [ -f "$ORCH/$f" ] && ok "$f" || fail "$f MISSING from bundle"
done
for f in bin/pi-workhorse.sh lib/quality-gate.sh lib/validate-exit-criteria.sh; do
    [ -x "$ORCH/$f" ] || fail "$f not executable"
done

# Every skill named in backticks after the word "skill" must be installed.
echo "  cross-references:"
for dir in "$CSK" "$PSK"; do
    [ -d "$dir" ] || continue
    for sk in "$dir"/*/SKILL.md; do
        [ -f "$sk" ] || continue
        name="$(basename "$(dirname "$sk")")"
        # references of the form: skill: `x`  /  skill `x`  /  **`x`** skill
        # and the prose reverse: `x` skill  /  `x` skills
        # (e.g. "feeds Claude's `verify-and-commit` skill" -- backticked name
        # BEFORE the word skill). Both orders name the same handoff; a check
        # that only sees one order would report OK while the other order's
        # reference is broken.
        refs="$(grep -oiE '(skills?:? +\*{0,2}`[a-z0-9-]+`|`[a-z0-9-]+`\*{0,2} +skills?)' "$sk" \
                | grep -oE '`[a-z0-9-]+`' | tr -d '`' | sort -u)"
        for r in $refs; do
            # No whitelist. A name written as a skill must resolve to an
            # installed skill. The earlier version excused "quality-gate" and
            # "validate-exit-criteria" here, which hid the fact that two skills
            # were calling bundled SCRIPTS "skills" -- a check softened to
            # accommodate a defect keeps the defect invisible.
            if [ -f "$CSK/$r/SKILL.md" ] || [ -f "$PSK/$r/SKILL.md" ]; then
                ok "$name -> $r"
            else
                fail "$name references skill '$r' which is not installed"
            fi
        done
        # scripts/ helpers the skill names
        for s in $(grep -oE 'scripts/[a-z0-9_-]+\.sh' "$sk" | sort -u); do
            p="$(dirname "$sk")/$s"
            if [ -x "$p" ]; then ok "$name -> $s"
            elif [ -f "$p" ]; then fail "$name -> $s exists but is not executable"
            else fail "$name references $s which is missing"; fi
        done
    done
done

echo
if [ "$bad" -eq 0 ]; then echo "DOCTOR: OK"; exit 0
else echo "DOCTOR: PROBLEMS FOUND"; exit 1; fi
