---
name: verify-and-commit
description: >
  Final phase of sdd-orchestrate. Read the load-bearing diff, confirm the
  architecture built matches the one specified, then commit. Never delegated —
  this is the judgement the cheaper model was not asked to supply.
---

# verify-and-commit

**Phase 7 of `sdd-orchestrate`.** Runs after `pi-workhorse.sh review` and
`validate`. Called by: `sdd-orchestrate`. Calls: nothing — this phase ends in a
commit.

This phase is Claude's and cannot be delegated. A workhorse may never commit.

## 1. Read the load-bearing diff

Not the whole diff — the parts where the spec's judgement lives:

- error paths, and anything that could fail **open**
- ordering and concurrency guarantees
- persistence and wire formats
- whatever the spec named a **key decision**
- the acceptance test the spec singled out

Mechanical churn is what the gate is for. But a green gate has repeatedly
coexisted with a broken feature, and every one of those was visible in the diff
to someone who read it.

## 2. Confirm the architecture, not just the tests

Does the thing that was built match the Architecture section? Tests passing is
necessary and not sufficient. Ask specifically:

- was any existing test weakened, skipped, `#[ignore]`d or deleted?
- does each new check assert on what the **consumer received**, or only on what
  the producing function returned?
- was a pinned fixture regenerated?
- did a requirement get satisfied in appearance only?

## 3. Check the reviewer's findings

`reviewer` reported PASS or FAIL. A FAIL is not advisory. A PASS is not proof —
verify anything it claims about the environment yourself. An agent's incidental
claim about its surroundings is not a measurement.

## 4. Handle a reported spec defect

If pi reported the spec is wrong, **read the code and decide yourself.** If it is
right, amend the spec (Claude's job, never pi's), record the correction in the
spec's Status line, and re-dispatch. Several requirements have been withdrawn
this way; every one of them was worth more than the code in that round.

## 5. Commit

Only after the above. Then:

```bash
git add -A <paths>
git commit -F -    # message body explaining WHY, not what
```

Commit the whole coherent change, not a partial one. Say what was wrong and why
this fixes it — the diff already says what changed.

Then update `HANDOFF.md` and run `pi-workhorse.sh handoff <root>`.

If the project has no diary yet:

```bash
scripts/new-handoff.sh <project_root>
```

It creates `HANDOFF.md` from `templates/HANDOFF-template.md` and never
overwrites an existing one.
