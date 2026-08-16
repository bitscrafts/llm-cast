#!/usr/bin/env bash
#
# usage.sh — report model, provider and token usage for a pi session, counting
# only usage not already reported (per-phase delta).
#
#   usage.sh <session_dir> [label] [--ledger <file>] [--clear] [--total]
#
# A session dir accumulates JSONL across phases AND across runs of the same
# spec, so naive summing double-counts. usage.sh keeps a ledger of how many
# lines of each session file it has already counted:
#
#   default — report only the lines added since the last call: the current
#             phase's own cost. Lines already counted stay counted, so a
#             phase that CONTINUES a session (repair -c appends to the same
#             file) is charged only for the lines it appended.
#   --clear  — reset the ledger, baselining everything already on disk as
#             counted. `run` calls this first, so a re-run of a spec reports
#             only that run's numbers, not the previous run's too. Prints
#             nothing.
#   --total  — report the whole current run: (tokens on disk now) minus the
#             --clear baseline. Without a baseline, the lifetime sum for the
#             spec.
#
# The ledger defaults to <session_dir>/.usage-ledger and is written under
# _tmp/, so it is never committed.
#
# Prints ONE line, so it can sit inside a run log without burying it. Prints
# nothing and exits 0 when there is nothing to report, so a caller can invoke
# it without guarding.
#
set -uo pipefail
SD="${1:-}"; LABEL="${2:-}"
LEDGER="" ; TOTAL=0; CLEAR=0
shift 2 2>/dev/null || true
while [ $# -gt 0 ]; do
  case "$1" in
    --ledger) LEDGER="$2"; shift 2 ;;
    --total)  TOTAL=1; shift ;;
    --clear)  CLEAR=1; shift ;;
    *) shift ;;
  esac
done

[ -d "$SD" ] || exit 0
LEDGER="${LEDGER:-$SD/.usage-ledger}"

mapfile -t FILES < <(find "$SD" -name '*.jsonl' -type f 2>/dev/null | sort)
[ "${#FILES[@]}" -gt 0 ] || exit 0

# Recorded state, if any: a `# baseline in out cr cw calls` marker line plus
# filename<TAB>lines counted for every file already reported. The baseline is
# per-field so `--total` can report this run's calls and in/out/cache
# consistently, not a lifetime calls figure next to a this-run token total.
BIN=0; BOUT=0; BCR=0; BCW=0; BCALLS=0; HAS_BASE=0
declare -A SEEN
if [ -f "$LEDGER" ]; then
  while IFS=$'\t' read -r f n; do
    case "$f" in
      \#\ baseline\ *)
        set -- $f; shift 2
        BIN=${1:-0}; BOUT=${2:-0}; BCR=${3:-0}; BCW=${4:-0}; BCALLS=${5:-0}; HAS_BASE=1
        ;;
      *) [ -n "$f" ] && SEEN["$f"]="$n" ;;
    esac
  done < "$LEDGER"
fi

# One awk for every mode. Sums usage objects on assistant/turn-end lines. NOTE:
# a message's `totalTokens` field is that call's context size, which grows
# through a session -- summing it double-counts. The billable quantity is
# input + output + cacheRead + cacheWrite summed across calls, which is what
# this sums. `cost` (if pi reports it) is a per-call USD figure.
AWK='{
  n = 0; pos = 1;
  while (match(substr($0, pos), /"usage":\{[^}]*\}/)) {
    u = substr($0, pos + RSTART - 1, RLENGTH); pos += RSTART + RLENGTH - 1; n++;
  }
  if (n == 0) next;
  if ($0 !~ /"type":"message"/ && $0 !~ /"type":"turn_end"/ && $0 !~ /"type":"message_end"/) next;
  ti = uo = ur = uw = 0;
  if (match(u, /"input":[0-9]+/))      ti = substr(u, RSTART+8,  RLENGTH-8);
  if (match(u, /"output":[0-9]+/))     uo = substr(u, RSTART+9,  RLENGTH-9);
  if (match(u, /"cacheRead":[0-9]+/))  ur = substr(u, RSTART+12, RLENGTH-12);
  if (match(u, /"cacheWrite":[0-9]+/)) uw = substr(u, RSTART+13, RLENGTH-13);
  IN += ti; OUT += uo; CR += ur; CW += uw; CALLS++;
  if (match($0, /"model":"[^"]*"/))    { m = substr($0, RSTART+9, RLENGTH-10); MODEL = m }
  if (match($0, /"provider":"[^"]*"/)) { p = substr($0, RSTART+12, RLENGTH-13); PROV = p }
  if (match($0, /"cost":\{[^}]*"total":[0-9.]+/)) {
    c = substr($0, RSTART, RLENGTH);
    if (match(c, /"total":[0-9.]+/)) COST += substr(c, RSTART+8, RLENGTH-8);
  }
}
END {
  printf "%d|%d|%d|%d|%d|%s|%s|%s", IN, OUT, CR, CW, CALLS, MODEL, PROV,
         (COST > 0 ? sprintf(" · $%.4f", COST) : "");
}'

TMP="$(mktemp "$SD/.usage-in.XXXXXX")"
trap 'rm -f "$TMP"' EXIT

# --clear: baseline everything on disk as already-counted. Emits nothing.
if [ "$CLEAR" -eq 1 ]; then
  for f in "${FILES[@]}"; do cat "$f" >> "$TMP"; done
  IFS='|' read -r IN OUT CR CW CALLS MODEL PROV COSTLINE < <(awk "$AWK" "$TMP")
  {
    echo "# baseline $IN $OUT $CR $CW $CALLS"
    for f in "${FILES[@]}"; do printf '%s\t%s\n' "$f" "$(wc -l < "$f")"; done
  } > "$LEDGER"
  exit 0
fi

# default: consume only the not-yet-counted lines, then advance the ledger.
# --total: fold every file into TMP and read the ledger only for its baseline.
NEW_LEDGER="${LEDGER}.new.$$"
if [ "$TOTAL" -eq 0 ]; then
  [ "$HAS_BASE" -eq 1 ] && echo "# baseline $BIN $BOUT $BCR $BCW $BCALLS" > "$NEW_LEDGER"
  for f in "${FILES[@]}"; do
    total=$(wc -l < "$f")
    off="${SEEN[$f]:-0}"
    if [ "$total" -gt "$off" ]; then
      tail -n +$((off + 1)) "$f" >> "$TMP"
    fi
    printf '%s\t%s\n' "$f" "$total" >> "$NEW_LEDGER"
  done
  mv "$NEW_LEDGER" "$LEDGER"
else
  for f in "${FILES[@]}"; do cat "$f" >> "$TMP"; done
fi

IFS='|' read -r IN OUT CR CW CALLS MODEL PROV COSTLINE < <(awk "$AWK" "$TMP")
[ "$CALLS" -eq 0 ] && exit 0

if [ "$TOTAL" -eq 1 ] && [ "$HAS_BASE" -eq 1 ]; then
  # This run = now minus the run-start baseline, per field, so calls and the
  # in/out/cache breakdown all describe the same run.
  sub() { local n="$1" b="$2"; [ "$n" -gt "$b" ] && echo $((n - b)) || echo "$n"; }
  IN="$(sub "$IN" "$BIN")"; OUT="$(sub "$OUT" "$BOUT")"
  CR="$(sub "$CR" "$BCR")"; CW="$(sub "$CW" "$BCW")"
  CALLS="$(sub "$CALLS" "$BCALLS")"
fi
tok=$((IN + OUT + CR + CW))

# One line. A five-line block per phase buries the run output it sits in.
printf "  [%s] %s · %d call%s · %d tok (in %s out %s cache %s)%s\n" \
  "${LABEL:-usage}" \
  "${MODEL:-?}" \
  "$CALLS" "$([ "$CALLS" -eq 1 ] && echo "" || echo "s")" \
  "$tok" "$IN" "$OUT" "$CR" "$COSTLINE"
