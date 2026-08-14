#!/usr/bin/env bash
#
# quality-gate.sh — the objective check. Ships WITH the bundle; depends on
# nothing in the target project.
#
#   quality-gate.sh [project_root]
#
# Detects the project type and runs its format / typecheck / lint / test
# stages. Exit 0 only if every stage passes. Prints one line per stage so the
# orchestrator can read the result without reading the code.
#
# A project MAY override by providing deploy/scripts/quality-gate.sh; if so it
# is used instead. That is an override, not a requirement -- the bundle works on
# a project that has never heard of it.
#
set -uo pipefail
ROOT="${1:-.}"
cd "$ROOT" 2>/dev/null || { echo "quality-gate: no such directory: $ROOT" >&2; exit 2; }

if [ -x "deploy/scripts/quality-gate.sh" ] && [ -z "${QG_NO_PROJECT_OVERRIDE:-}" ]; then
    echo "quality-gate: using project override deploy/scripts/quality-gate.sh"
    exec bash deploy/scripts/quality-gate.sh "$ROOT"
fi

FAILED=0
stage() {  # stage <name> <cmd...>
    local name="$1"; shift
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '  %-28s SKIP (%s not installed)\n' "$name" "$1"; return
    fi
    local out
    if out="$("$@" 2>&1)"; then
        printf '  %-28s PASS\n' "$name"
    else
        printf '  %-28s FAIL\n' "$name"
        echo "$out" | tail -40
        FAILED=1
    fi
}

detected=""
if [ -f Cargo.toml ]; then
    detected="rust"
    PKG=""; [ -n "${QG_CARGO_PKG:-}" ] && PKG="-p ${QG_CARGO_PKG}"
    stage "cargo fmt --check"        cargo fmt $PKG -- --check
    stage "cargo check"              cargo check $PKG
    stage "cargo clippy -D warnings" cargo clippy $PKG -- -D warnings
    stage "cargo test"               cargo test $PKG -j 2
elif [ -f package.json ]; then
    detected="node"
    RUN="npm"; [ -f pnpm-lock.yaml ] && RUN="pnpm"; [ -f yarn.lock ] && RUN="yarn"
    has() { node -e "process.exit(require('./package.json').scripts?.['$1']?0:1)" 2>/dev/null; }
    has lint  && stage "$RUN run lint"  "$RUN" run lint
    has build && stage "$RUN run build" "$RUN" run build
    has test  && stage "$RUN test"      "$RUN" test
elif [ -f pyproject.toml ] || [ -f setup.py ] || ls ./*.py >/dev/null 2>&1; then
    detected="python"
    # Python always runs through uv -- it resolves the project environment
    # rather than whatever happens to be on PATH, which is the difference
    # between "passes here" and "passes anywhere".
    if command -v uv >/dev/null 2>&1; then
        stage "uv run ruff"   uv run ruff check .
        stage "uv run mypy"   uv run mypy .
        stage "uv run pytest" uv run pytest -q
    else
        echo "  uv not installed -- required for Python projects" >&2
        echo "  install: curl -LsSf https://astral.sh/uv/install.sh | sh" >&2
        FAILED=1
    fi
elif [ -f go.mod ]; then
    detected="go"
    stage "gofmt"    gofmt -l .
    stage "go vet"   go vet ./...
    stage "go test"  go test ./...
else
    echo "quality-gate: no recognised project type in $ROOT" >&2
    echo "  supported: Cargo.toml, package.json, pyproject.toml/setup.py/*.py, go.mod" >&2
    echo "  or provide deploy/scripts/quality-gate.sh" >&2
    exit 2
fi

echo
if [ "$FAILED" -eq 0 ]; then
    echo "QUALITY GATE: PASSED ($detected)"; exit 0
else
    echo "QUALITY GATE: FAILED ($detected)"; exit 1
fi
