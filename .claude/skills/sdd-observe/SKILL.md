---
name: sdd-observe
description: >
  Watch a pi-orchestration run end to end — planner (pi-plan), workhorse
  (pi-workhorse), gate, validation — without touching the tree. Intervene only
  on defined triggers. Use when a spec is being planned or implemented by pi
  workers and the orchestrator must observe and analyse.
---

# sdd-observe

**Role**: the orchestrator (Hermes, Claude, or any agent) WATCHES the
pi-plan → pi-work interaction. The workers do the work; the observer records,
analyses, and intervenes only on the triggers below. Touching the tree while a
worker runs corrupts the diff the workhorse's no-commit check and the
reviewer's read depend on.

**The flow observed**:

```
brief → pi-plan.sh (planner tier model) → specs/NN-<feature>[-partM].md
      → pi-workhorse.sh run <part> → implement → gate → repair → review
      → validate (exit criteria) → HANDOFF diary
      → observer: read diff, commit, record, analyse
```

## Procedure

1. **Stage the workers visibly** (optional but recommended): each planner and
   workhorse invocation gets its own herdr tab in the TV-mirrored session, so
   the operator watches live. `herdr --session tv-demo tab create --cwd
   <root> --label plan-<feature>` / `work-<part>`; close the tab when the
   phase ends.
2. **Watch, don't poll.** `herdr agent list` / `herdr api snapshot` show
   working→idle transitions per pane; `herdr agent read <pane>` / `herdr pane
   read <pane>` harvest output. The workhorse also prints phase lines
   (`[implement] -> model ...`, usage) on stderr — capture those.
3. **Record each phase** in agent-memory (or the project's handoff doc):
   planner output (parts written), per-part gate results, costs. Store at
   phase end, search before storing, dedupe.
4. **Intervene ONLY on these triggers:**
   - **Stall**: a workhorse turn ended and `implementation_changed` is false
     (only HANDOFF/_tmp edited) — the spec-03 lesson. Escalate or re-brief.
   - **Gate failing past ORCH_MAX_REPAIR_ROUNDS** (exit 3): read the gate
     output; decide re-spec vs escalate.
   - **SPEC-DEFECT** marker (exit 5): the worker reports the spec itself is
     wrong. Read the spec, fix it (or re-plan), do not repair the code.
   - **Exit 6** (implement/plan produced nothing): re-brief.
   - **Exit 7** (worker COMMITTED): unwind the commit immediately, then
     investigate how the worker wrote git state.
   - **Exit criteria unmet** (exit 4): read validation output; decide whether
     the spec or the implementation is wrong.
5. **Never commit for the worker mid-run.** Phase 7 belongs to the observer:
   after all parts validate, read the diff, commit, update HANDOFF.

## Analysis loop (the value of observing)

- After each run: what did the worker's usage report say (model, tokens,
  cost)? Did the gate catch anything the unit tests missed? Compare against
  the previous part — drift detection.
- On-device/on-TM findings (content-type quirks, refresh behaviour, glyph
  order) are exactly what unit tests cannot catch: record them as
  milestone findings, not as test failures.
- Store a day-summary per session in agent-memory (topic
  `workspace/<project>/<date>-day-summary`, importance 0.8) so later sessions
  start with yesterday's conclusions.

## Pitfalls

- **Observation mode is read-only.** Do not edit files, run gates, or commit
  while a worker runs. The other session's rule applies: watch and analyse.
- The workhorse passes ALL pi/skills/* to every invocation — if the bundle
  carries a planner skill, ensure the workhorse's skill list is explicit
  (implementer+reviewer only) so planning instructions never leak into
  implementation.
- A `herdr tab create` on the TV session is non-destructive, but close the
  tab when the phase ends — a dead worker tab clutters the operator's view.
- TV display relaunches (set_font_size/mirror_session) drop the cast session:
  re-cast `http://10.10.10.217:18080/live.m3u8` (contentType
  `application/vnd.apple.mpegurl`) after any display tool call.
