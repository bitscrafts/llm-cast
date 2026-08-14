#!/usr/bin/env bash
#
# usage.sh — report model, provider and token usage for a pi session.
#
#   usage.sh <session_dir> [label]
#
# Reads the session JSONL pi writes under --session-dir and sums the per-call
# usage. NOTE: the `totalTokens` field on each message is that call's context
# size, which grows through a session -- summing it double-counts badly. The
# billable quantity is input + output + cacheRead + cacheWrite summed across
# calls, which is what this reports.
#
# Prints ONE line, so it can sit inside a run log without burying it. Prints
# nothing and exits 0 if no session is found, so a caller can always invoke it
# without guarding.
#
set -uo pipefail
SD="${1:-}"; LABEL="${2:-}"
[ -d "$SD" ] || exit 0
# ALL session files under the dir, not just the newest. Each phase that does
# not continue a session starts a new file, so reading only the last one
# reported the review's usage as the task total and silently dropped
# implement's -- which was the larger half.
mapfile -t FILES < <(find "$SD" -name '*.jsonl' -type f 2>/dev/null | sort)
[ "${#FILES[@]}" -gt 0 ] || exit 0

awk -v label="$LABEL" '
{
  # one usage object per assistant message; take the LAST occurrence per line
  # (streaming lines repeat a partial, the final one is authoritative)
  n = 0; pos = 1;
  while (match(substr($0, pos), /"usage":\{[^}]*\}/)) {
    u = substr($0, pos + RSTART - 1, RLENGTH); pos += RSTART + RLENGTH - 1; n++;
  }
  if (n == 0) next;
  # Session JSONL uses {"type":"message",...}; the stdout stream uses
  # turn_end/message_end. Accept either, so this works on both.
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
  if (CALLS == 0) exit 0;
  # One line. A five-line block per phase buries the run output it sits in.
  printf "  [%s] %s · %d call%s · %s tok (in %s out %s cache %s)%s\n",
         (label == "" ? "usage" : label),
         (MODEL == "" ? "?" : MODEL),
         CALLS, (CALLS == 1 ? "" : "s"),
         IN+OUT+CR+CW, IN, OUT, CR,
         (COST > 0 ? sprintf(" · $%.4f", COST) : "");
}
' "${FILES[@]}"
