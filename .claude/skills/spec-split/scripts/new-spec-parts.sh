#!/usr/bin/env bash
#
# new-spec-parts.sh — emit part-N spec skeletons deterministically.
#
#   new-spec-parts.sh <NN> "<title>" <project_root> <parts> [source]
#
# Splitting is a first-class step in the workflow (spec-split skill): the
# skeletons come out of here, never from a model, so every part has the same
# six-section structure as a full spec. Content is model-filled afterwards.
#
# Writes <project_root>/specs/<NN>-<slug>-part1.md .. -partN.md and prints the
# paths. Refuses to overwrite: a spec is a contract, not a scratch file.
#
set -euo pipefail
NN="${1:?usage: new-spec-parts.sh <NN> \"<title>\" <project_root> <parts> [source]}"
TITLE="${2:?title required}"
ROOT="${3:?project_root required}"
N="${4:?parts count required}"
SOURCE="${5:-manual}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Reuse the canonical template from the spec-author skill next door, so a
# part's structure is byte-identical to a full spec's.
TPL="$(cd "$HERE/../../spec-author/templates" && pwd)/spec-template.md"
[ -f "$TPL" ] || { echo "template missing: $TPL" >&2; exit 2; }

SLUG="$(printf '%s' "$TITLE" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')"
mkdir -p "$ROOT/specs"
for i in $(seq 1 "$N"); do
    OUT="$ROOT/specs/${NN}-${SLUG}-part${i}.md"
    [ -e "$OUT" ] && { echo "refusing to overwrite existing spec: $OUT" >&2; exit 1; }
    sed -e "s|<project> — spec-NN: <one-line title>|$(basename "$ROOT") — spec-${NN}: ${TITLE} (part ${i}/${N})|" \
        -e "s|\`/abs/path\`|\`$(cd "$ROOT" && pwd)\`|" \
        -e "s|- \*\*Source\*\*: <what prompted this: audit finding, defect, plan item>|- **Source**: ${SOURCE} ($(date +%Y-%m-%d))|" \
        "$TPL" > "$OUT"
    echo "$OUT"
done
