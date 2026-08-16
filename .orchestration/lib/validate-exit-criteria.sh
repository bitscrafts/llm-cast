#!/usr/bin/env bash
#
# validate-exit-criteria.sh — run a spec's Exit Criteria and report each one.
# Ships WITH the bundle; depends on nothing in the target project.
#
#   validate-exit-criteria.sh <spec.md> [project_root]
#
# Grammar (this is why spec-author mandates it):
#   - [ ] `command` — description
#   - [x] `command` — description        (already-satisfied items are still run)
#
# The FIRST backticked span on the line is the command. It must exit 0 when the
# criterion is SATISFIED. A check that should find nothing must therefore be
# written `! grep -q pattern path` -- a bare `grep -q` exits 1 on success and
# will be reported FAIL. This bit three of the first nine criteria it was run
# against.
#
# Every checkbox item under "## Exit Criteria" whose text contains a backticked
# command has that command executed in the project root. Exit 0 only if all
# pass. Items with no backticked command are reported as MANUAL and do not fail
# the run -- but they are counted, so a spec made of prose is visibly unchecked.
#
# Section parsing is fence-aware and terminates only on a line that is an h1/h2
# heading: a `###` sub-heading is content. A naive `find("##")` here once
# dropped a third of every spec silently.
#
set -uo pipefail

# Toolchain PATH bootstrap (same policy as quality-gate.sh): criteria such as
# `cargo test` must resolve in non-interactive shells where rustup's bin dir
# is not on the default PATH.
for _tb in "${CARGO_HOME:-$HOME/.cargo}/bin" "$HOME/.cargo/bin"; do
    if [ -d "$_tb" ] && ! case ":$PATH:" in *":$_tb:"*) ;; *) false ;; esac; then
        PATH="$_tb:$PATH"
    fi
done
unset _tb

# python3 is required for exit-criteria extraction; fail loudly when absent
# rather than letting the pipeline produce an empty item list (fail-open).
command -v python3 >/dev/null 2>&1 || {
    echo "validate-exit-criteria: python3 is required (criteria extraction)" >&2
    exit 2
}

SPEC="${1:?usage: validate-exit-criteria.sh <spec.md> [project_root]}"
ROOT="${2:-$(dirname "$SPEC")/..}"
[ -f "$SPEC" ] || { echo "no such spec: $SPEC" >&2; exit 2; }
cd "$ROOT" 2>/dev/null || { echo "no such project root: $ROOT" >&2; exit 2; }

# The `grep -v` on the closing line is load-bearing. Python's '\n'.join([])
# prints ONE empty line, so mapfile produced a single empty element, the
# "no criteria" guard below never fired, and the phantom item was reported as
# MANUAL -- which does not fail the run. A spec with zero machine-checkable
# criteria therefore passed validation with exit 0: fail-open, in the tool
# whose entire job is to fail closed. Found running it against a real
# non-conforming spec.
mapfile -t ITEMS < <(python3 - "$SPEC" <<'PY' | grep -v '^[[:space:]]*$'
import sys, re
lines = open(sys.argv[1], encoding='utf-8').read().split('\n')
fence = False; inside = False; out = []
for ln in lines:
    st = ln.lstrip()
    if st.startswith('```') or st.startswith('~~~'):
        fence = not fence
        continue
    if fence:
        continue
    if re.match(r'^##\s+Exit Criteria\s*$', ln):
        inside = True; continue
    if inside and re.match(r'^#{1,2}\s', ln):
        break
    if inside and re.match(r'^\s*-\s*\[[ xX]\]', ln):
        out.append(ln.strip())
print('\n'.join(out))
PY
)

[ "${#ITEMS[@]}" -eq 0 ] && {
    echo "NO CHECKBOX EXIT CRITERIA in $SPEC" >&2
    echo "  Exit Criteria must be '- [ ] \`command\`' items; prose cannot be checked." >&2
    exit 2
}

pass=0; fail=0; manual=0; n=0
for item in "${ITEMS[@]}"; do
    n=$((n+1))
    # First backticked span is the command. Extracted with python3 (not
    # grep -oP): -P is GNU-only and breaks on BSD/macOS grep.
    cmd="$(printf '%s' "$item" | python3 -c 'import sys,re; m=re.search(r"`([^`]*)`", sys.stdin.read()); print(m.group(1) if m else "")')"
    label="$(printf '%s' "$item" | sed 's/^\s*-\s*\[[ xX]\]\s*//' | cut -c1-72)"
    if [ -z "$cmd" ]; then
        printf '  %2d MANUAL  %s\n' "$n" "$label"; manual=$((manual+1)); continue
    fi
    if out="$(bash -c "$cmd" 2>&1)"; then
        printf '  %2d PASS    %s\n' "$n" "$label"; pass=$((pass+1))
    else
        printf '  %2d FAIL    %s\n' "$n" "$label"
        printf '            $ %s\n' "$cmd"
        printf '%s\n' "$out" | head -6 | sed 's/^/            /'
        fail=$((fail+1))
    fi
done

echo
echo "EXIT CRITERIA: $pass passed, $fail failed, $manual manual (of $n)"
[ "$fail" -eq 0 ] || exit 1
[ "$manual" -eq 0 ] || echo "note: $manual criterion(s) need a human; they did not fail the run."
exit 0
