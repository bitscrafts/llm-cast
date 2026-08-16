---
name: sdd-orchestrate
description: >
  Entry point for spec-driven development where Claude plans and verifies and
  `pi` implements. Use for any non-trivial change. Drives all phases and calls
  the other skills in order — spec-author, pi-delegate, verify-and-commit.
---

# sdd-orchestrate

**The master loop. Start here.** Every other skill in this bundle is invoked
from a phase below; do not call them out of order.

Claude plans and verifies. `pi` implements. The split exists because
implementation reasoning is the expensive part, and the cheapest competent model
that passes the gate is the right one to do it — one session spent **1.9M tokens**
on Claude subagents for work `pi` could have done under a written contract.

## Phases, and the skill each one calls

| # | phase | who | skill / command |
|---|---|---|---|
| 0 | orient | Claude | read `HANDOFF.md`, then `specs/` |
| 1 | author the contract | Claude | **→ skill: `spec-author`** |
| 2 | implement | pi | `pi-workhorse.sh implement <spec>` **→ pi skill: `implementer`** |
| 3 | gate | shell | `pi-workhorse.sh gate <root>` — no model |
| 4 | repair (loop) | pi | `pi-workhorse.sh repair <spec>` — escalate after N rounds |
| 5 | review | pi | `pi-workhorse.sh review <spec>` **→ pi skill: `reviewer`** |
| 6 | validate | shell | `pi-workhorse.sh validate <spec>` — no model |
| 7 | verify & commit | Claude | **→ skill: `verify-and-commit`** |
| 8 | close out | pi/Claude | `pi-workhorse.sh handoff <root>` |

**→ skill: `pi-delegate`** covers phases 2–6 in detail: tool grants, sessions,
escalation policy and token discipline. Read it before running them.

## Phase 0 — orient

```bash
cat <root>/HANDOFF.md        # what the last session left, and what not to repeat
ls <root>/specs/
```

Never start without reading `HANDOFF.md`. A blocker recorded there is worth more
than a blocker rediscovered.

## Phase 1 — author the contract

Invoke **`spec-author`**. Do not proceed until the spec has all six sections and
every Exit Criterion is a runnable `- [ ]` command.

The spec carries the judgement so the workhorse needs none. Every ambiguity left
in it is a decision a cheaper model will make for you.

## Phases 2–6 — one command

```bash
pi-workhorse.sh run specs/NN-name.md <root>
```

That drives implement → gate → repair (up to `ORCH_MAX_REPAIR_ROUNDS`) →
escalate → review → validate, and **stops the moment something needs
judgement**: a reported spec defect (exit 5), a gate still failing after
escalation (exit 3), unmet exit criteria (exit 4), an implement that
produced nothing even after escalation (exit 6), or a workhorse that
COMMITTED — detected mechanically by HEAD movement, since the no-commit rule
is prompt-level only (exit 7).

**Escalation triggers on a stalled implement too**, not only on gate failure.
A phase 2 that times out or leaves the tree unchanged is retried once on
`ORCH_ESCALATION_MODEL` before the loop gives up — without that, a workhorse
that writes nothing never reaches the gate and the repair ladder never engages.

Read **`pi-delegate`** for what each phase does and when to override. The
individual subcommands remain available for running a phase by hand.

**Why this is a shell loop and not a Claude subagent.** Phases 2–6 are a state
machine: run a command, check an exit code, loop, escalate. A subagent driving
it would spend Claude tokens re-reading gate output and pi's replies — the exact
cost this bundle exists to remove. The only genuine judgement in the middle is
*"is pi's spec-defect report real?"*, and answering that needs the orchestrator's
project context, not an isolated subagent's. So the loop is deterministic and
hands back to you when judgement is required.

Stop and think whenever pi reports the **spec** is wrong. That report is the most
valuable thing it produces — verify the claim against the code yourself, then
amend the spec. Do not let pi amend it.

## Phase 7 — verify and commit

Invoke **`verify-and-commit`**. This phase is Claude's and cannot be delegated:
reading the load-bearing diff, judging whether the architecture built matches the
one specified, and committing.

## Phase 8 — close out

```bash
pi-workhorse.sh handoff <root>     # fails if HANDOFF.md is missing or stale
```

## The rule behind all of it

A green gate has repeatedly coexisted with a broken feature, because the check
exercised the unit and never the seam. Automate what is objective — the gate, the
exit criteria, the handoff check — and spend human-grade attention on the seam.
