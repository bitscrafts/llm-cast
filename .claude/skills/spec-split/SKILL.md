---
name: spec-split
description: >
  Split a spec too large for one workhorse pass into sequential, independently
  validatable part specs. Use after authoring a spec whose "Files to Modify"
  spans more than one implement-gate cycle can land, or after a workhorse stall.
  Called by: spec-author.
---

# spec-split

**A sub-step of `spec-author`.** Called by: `spec-author`.
Next phase: **`pi-delegate`** (implementation), dispatched once per part, in order.

A spec that is too large to implement in one workhorse pass does not fail
loudly; it stalls. The stall signature is a long research phase and no file
writes — spec-03 run #2 was 52 tool calls and 0 files written: the workhorse
curl-downloaded rmcp source, read it in sed slices, then handed off "next agent
starting position". The harness now escalates a stall, but the escalated pass is
still too large. The split is what turns one stalled mega-run into a chain of
landed increments.

## When to split

Split when any of these is true:

- The harness exited 6: "spec is probably too large for a workhorse on this
  repo. Narrow it."
- An implement turn ran its research budget and wrote nothing (the stall
  signature above).
- The spec's "Files to Modify" adds several new modules/dirs — a whole module
  tree plus a test target plus a bin is usually too much for one pass — or its
  TDD Contract has both unit tests and a process-spawning E2E surface.
- You cannot confidently say one workhorse pass (≤ `ORCH_MAX_TOOL_ITERATIONS`
  tool calls, ≤ `ORCH_PHASE_TIMEOUT` wall-clock) can both write the code and
  pass the gate.

When in doubt, split. A landed small part beats a stalled big one, and parts
are cheaper to review than one megadiff.

## Split rules (binding)

1. **Every part leaves a green, compiling tree.** Part N must build and pass
   its own gate and its own exit criteria standalone. No part may reference a
   file a later part creates — that is the rule that keeps each part
   independently reviewable and committable.
2. **Dependencies before dependents.** Order parts: dependency lines in
   `Cargo.toml` and module roots in `lib.rs` first; then core modules in
   dependency order; then the tool/server surface; the E2E/acceptance test
   last, in its own part. A dependency conflict (e.g. rmcp vs a pinned rustix)
   must surface in part 1, before code is written on top of it.
3. **Shared infrastructure lands where its subjects exist.** The meta-test
   count that walks new dirs goes in the part that creates those dirs, so the
   no-unwrap guarantee applies from the first part onward. An exit criterion
   may only name paths that exist at that part's end: `! grep -q pattern path`
   over a missing path exits 2 and inverts to a false FAIL.
4. **Each part is a full six-section spec**, generated with
   `spec-split/scripts/new-spec-parts.sh` (never hand-rolled), numbered
   `<NN>-<name>-partN.md`, carrying:
   - its own Requirements, restated in that part's scope and mapped back to the
     master spec's R-numbers so the trail is traceable;
   - its own machine-checkable Exit Criteria (`- [ ]` + backticked command
     first, exit 0 when SATISFIED, `[1-9]` non-vacuity on test filters — see
     `spec-author`);
   - its own Files to Modify table naming only what that part creates;
   - the master spec's Guardrails, adapted — drop any that name files the part
     does not touch.
5. **The master spec stays the source of truth.** Never delete or rewrite it;
   add a one-line pointer listing the parts. The parts are derived contracts.
6. **Dispatch is sequential and verified.** Implement part 1 end-to-end
   (implement → gate → repair → review → validate) and the orchestrator COMMITS
   it before part 2 is dispatched. Never dispatch part N+1 while part N is
   un-landed.
7. **Escalate the model WITH the split, not instead of it.** If the stall was
   model-driven (research-hoarding), dispatch the parts with the escalation
   model as the base model, so the implement phase itself is the strong model —
   not only the repair fallback:
   `ORCH_MODEL=$ORCH_ESCALATION_MODEL ORCH_PROVIDER=$ORCH_ESCALATION_PROVIDER
   pi-workhorse.sh run <part> <repo>`. The split bounds scope; the escalation
   model provides competence; you need both.

## Doing the split

1. Read the master spec fully; list its "Files to Modify".
2. Draw the dependency DAG among those files: roots (`Cargo.toml`, `lib.rs`) →
   core modules → surface → tests. Partition into contiguous slices, each
   leaving a green tree, each no bigger than one workhorse pass.
3. Generate the skeletons:
   `spec-split/scripts/new-spec-parts.sh <NN> "<title>" <root> <parts> [source]`
4. Fill each part's six sections. Restate requirements, subset the TDD Contract
   (the acceptance test stays in the final part), write that part's exit
   criteria, carry the guardrails, list that part's files.
5. Point the master spec at the parts; commit the parts (orchestrator-only).
6. Dispatch part 1; verify it lands; repeat.

## Worked precedents

- spec-01 (cast-tv-terminal): split into 6 parts before dispatch.
- spec-03 (mcp-server, 23 KB): stalled twice (run #2: 52 calls, 0 writes),
   then split into 3 parts — part 1 mux core + mcp foundation + unit tests,
   part 2 server surface + bin, part 3 E2E + acceptance — and re-dispatched
   with `ORCH_MODEL` set to the escalation model (glm-5.2 via nvidia).
