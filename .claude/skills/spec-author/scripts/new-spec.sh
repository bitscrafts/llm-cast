#!/usr/bin/env bash
#
# new-spec.sh — emit a spec skeleton deterministically.
#
#   new-spec.sh <NN> "<title>" <project_root> [source]
#
# The STRUCTURE is produced here, never by a model. The model fills content
# only. That is what makes spec shape reproducible instead of a matter of how
# well the template was paraphrased on the day.
#
# Writes <project_root>/specs/<NN>-<slug>.md and prints the path. Refuses to
# overwrite: a spec is a contract, not a scratch file.
#
set -euo pipefail
NN="${1:?usage: new-spec.sh <NN> \"<title>\" <project_root> [source]}"
TITLE="${2:?title required}"
ROOT="${3:?project_root required}"
SOURCE="${4:-manual}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TPL="$HERE/../templates/spec-template.md"
[ -f "$TPL" ] || { echo "template missing: $TPL" >&2; exit 2; }

SLUG="$(printf '%s' "$TITLE" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')"
OUT="$ROOT/specs/${NN}-${SLUG}.md"
[ -e "$OUT" ] && { echo "refusing to overwrite existing spec: $OUT" >&2; exit 1; }
mkdir -p "$ROOT/specs"

sed -e "s|<project> — spec-NN: <one-line title>|$(basename "$ROOT") — spec-${NN}: ${TITLE}|" \
    -e "s|\`/abs/path\`|\`$(cd "$ROOT" && pwd)\`|" \
    -e "s|- \*\*Source\*\*: <what prompted this: audit finding, defect, plan item>|- **Source**: ${SOURCE} ($(date +%Y-%m-%d))|" \
    "$TPL" > "$OUT"
echo "$OUT"
