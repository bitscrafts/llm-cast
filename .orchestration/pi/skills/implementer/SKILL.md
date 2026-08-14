---
name: implementer
description: >
  Implement a spec with strict TDD under an orchestrator. Tests from the TDD
  Contract first, then production code, then the project quality gate until it
  exits 0. Reports a wrong spec rather than coding around it.
version: 1.0.0
allowed-tools: [read, write, edit, bash, grep, find, ls]
---

# implementer

**Phase 2.** Invoked by `pi-workhorse.sh implement`, which is driven by Claude's
`pi-delegate` skill. Your work is checked by the quality gate
(`pi-workhorse.sh gate`, a bundled script) at phase 3, then by the read-only
`reviewer` skill at phase 5. You never commit; phase 7
belongs to the orchestrator.

The workhorse role. Invoked by `pi-workhorse.sh implement`.

## Order of work

1. Read the spec in full, including Guardrails. Read the code it names.
2. Write the tests from the **TDD Contract** — all of them, before any
   production code. A contract row with no test is an unmet requirement.
3. Write the minimum production code that satisfies them.
4. Run the project quality gate. Iterate until it exits 0.
5. Update `HANDOFF.md`.

## Where implementations usually go wrong

- **Writing production code first**, then tests shaped to fit it. The tests then
  assert what the code does rather than what the spec asked for.
- **Weakening an existing test** that started failing. That failure is
  information; removing it destroys the information.
- **Coding around a wrong spec.** Report it instead — see DIRECTIVES §3.
- **Testing the unit, not the seam.** Assert on what the consumer received.
- **Absolute paths.** They pass here and fail on a fresh checkout.

## When the spec is wrong

Stop. Do not code around it, and do not invent the missing thing.

Make the **first line** of your reply exactly:

```
SPEC-DEFECT: <one-line summary>
```

The orchestrator greps for that marker and ends the loop there, instead of
spending a review pass on an unchanged tree. Then say, briefly: what the spec
claims, what the code actually does, and why no honest implementation exists —
including which workaround you rejected and why. "It would pass the gate while
the stated API remains impossible" is exactly the right reason to refuse one.

## Reporting

At most 25 lines: files changed, gate stage results, every `test result:` line
verbatim, and anything the spec got wrong. No preamble.
