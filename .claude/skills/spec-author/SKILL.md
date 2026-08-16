---
name: spec-author
description: >
  Write a spec that a cheaper model can implement correctly without judgement
  calls. Use before delegating any non-trivial change via pi-delegate. Covers
  the six mandatory sections, verified premises, and exit criteria that a
  machine can check.
---

# spec-author

**Phase 1 of `sdd-orchestrate`.** Called by: `sdd-orchestrate`.
Next phase: **`pi-delegate`** (implementation). Do not proceed to it until every
Exit Criterion here is a runnable `- [ ]` command.

The spec IS the contract. It carries the judgement so the workhorse does not
have to supply any.

This matters more when the implementer is a cheaper model: **every ambiguity you
leave is a decision it will make for you.** Published failure analysis puts 42%
of multi-agent failures on specification ambiguity, and in practice those errors
belong to the orchestrator, not the worker.

## Verify every premise before writing it

Do not write "X currently does Y" unless you have read X. Requirements have had
to be withdrawn mid-flight for being premised on a misreading — an API assumed
synchronous that was async, a field assumed serialized that was not, a lock that
would have deadlocked by design.

Read the code the spec talks about. Grep for the call sites. Then write.

## The six sections

1. **Overview** — the why, and the defect or driver in concrete terms.
2. **Requirements** — numbered, each independently checkable. Functional and
   non-functional separated.
3. **Architecture** — modules, data flow, and **key decisions with rationale**.
   State what was rejected and why; that is what stops a reimplementation.
4. **TDD Contract** — one row per behaviour: id, test name, given, expects.
   Name the **acceptance test** explicitly, and say which requirement is the one
   a plausible implementation gets wrong.
5. **Exit Criteria** — `- [ ]` checkbox items, each a runnable command in
   backticks. This grammar is what `validate-exit-criteria` and `split` consume;
   prose criteria cannot be machine-checked.
6. **Guardrails** — what the implementer must NOT do, and the error-handling
   expectations. Say why, briefly: a rule with a reason survives paraphrase.

## Make the dangerous requirement obvious

For each spec, ask: *which requirement will a plausible implementation satisfy
in appearance only?* Name it, and give it a test that fails on the naive version.

Where it matters, require the implementer to **write the wrong version first,
watch the test fail, then fix it** — and to paste both outputs. That is the only
proof the test can detect the defect at all.

## Generate the skeleton; never hand-roll it

```bash
scripts/new-spec.sh <NN> "<title>" <project_root> [source]
```

The structure comes from `templates/spec-template.md` via that script, so every
spec has the same six sections in the same order. You fill content only. Refuses
to overwrite: a spec is a contract, not a scratch file.

## When the spec is too large — split it

A spec whose "Files to Modify" spans more than one implement-gate cycle will
not fail loudly on the workhorse; it will stall and hand off. If the harness
exited 6, a workhorse pass stalled on it, or you cannot confidently say one
pass can write the code and pass the gate, invoke the `spec-split` skill before
delegating. Splitting is a first-class step in this workflow, not a workaround
for a big task: each part leaves a green tree, each is independently
reviewable and committable, and the parts are dispatched strictly in order.

## Exit criteria must be machine-checkable

Every requirement needs a criterion, and no criterion may be subjective. "Code
looks good" is not a criterion. Prefer a shell command with an exit status.

Include the negative checks — no hardcoded absolute paths, no forbidden
dependency, the pinned fixture hash unchanged.

**The command must exit 0 when the criterion is SATISFIED.** This is the trap:
a check that should find nothing must be written `! grep -q pattern path`. A
bare `grep -q` exits 1 when it finds nothing, so the validator reports FAIL on
a passing project. Three of the first nine criteria run through
`lib/validate-exit-criteria.sh` were wrong this way.

**A test-filter criterion that matches zero tests passes vacuously.** `cargo
test --quiet <filter>` on a filter that matches nothing still prints
`test result: ok. 0 passed; 0 filtered out` — so a criterion that greps for
`test result: ok` is green before the tests exist. Require a non-zero count:
`cargo test --quiet <filter> 2>&1 | grep -qE "^test result: ok\. [1-9]"`.
(Found in the field: spec-04's week/round-trip criteria passed on an
unimplemented crate for exactly this reason.)

The first backticked span on the line is the command. Put it first.

## Guardrails that have proven necessary

Carry these forward unless there is a reason not to: never edit the spec; never
commit; never weaken or delete a test; never regenerate a pinned fixture; report
a wrong premise rather than coding around it; raw output, never summed totals.

## Length is not the goal

A spec is long enough when a competent implementer needs no judgement calls.
Beyond that, more words dilute the binding parts.
