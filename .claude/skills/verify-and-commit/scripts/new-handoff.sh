#!/usr/bin/env bash
#
# new-handoff.sh — create HANDOFF.md from the template if absent.
#
#   new-handoff.sh <project_root>
#
# Never overwrites an existing diary. Prints the path either way, so the caller
# can open it and fill in the sections.
#
set -euo pipefail
ROOT="${1:?usage: new-handoff.sh <project_root>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TPL="$HERE/../templates/HANDOFF-template.md"
OUT="$ROOT/HANDOFF.md"
if [ -e "$OUT" ]; then echo "$OUT (exists, untouched)"; exit 0; fi
[ -f "$TPL" ] || { echo "template missing: $TPL" >&2; exit 2; }
cp "$TPL" "$OUT"; echo "$OUT (created)"
