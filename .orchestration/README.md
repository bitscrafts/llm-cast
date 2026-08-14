# pi-orchestration

**Claude Code plans and verifies. `pi` implements.**

A self-contained bundle for spec-driven development where the expensive model
writes the contract and reviews the result, and a cheaper model does the typing.
One project spent **1.9M tokens** on Claude subagents for work `pi` could have
done under a written spec.

Nothing here depends on the host's `CLAUDE.md`, `AGENTS.md`, or shell aliases.
Unzip, run `install.sh`, and the loop works on any machine.

---

## Install

```bash
unzip pi-orchestration-*.zip && cd bundle

# user-wide — every project on this machine
./install.sh
export PATH="$PATH:$HOME/.pi-orchestration/bin"
export ORCH_HOME="$HOME/.pi-orchestration"

# or project-local — vendored into one repo, no env vars needed
./install.sh --project /path/to/repo
```

**Project-local wins.** The harness walks up from the working directory looking
for `.orchestration/config.env` before falling back to the user-wide install, so
a checkout carries its own rules and two repos can run different versions. Commit
`.orchestration/` and `.claude/skills/` to pin them.

pi auto-discovers only its user-wide skills directory, so for a project-local
install the harness passes each pi skill explicitly with `--skill` (which accepts
a directory and repeats).

Idempotent — a differing existing file is backed up, never silently replaced.

**Requires**: `pi` on `PATH` (or `PI_BIN` set) with provider credentials
configured. Nothing else. The quality gate and the exit-criteria validator ship
inside the bundle and detect the project type themselves — **the target project
needs no scripts of its own.**

For Python projects, `uv` must be installed: every Python stage runs through
`uv run`, so the gate uses the project's resolved environment rather than
whatever happens to be on `PATH`.

---

## Start here

In Claude Code, invoke the **`sdd-orchestrate`** skill. It drives every phase and
calls the others in order.

Phases 2–6 are one command:

```bash
pi-workhorse.sh run specs/NN-name.md <project_root>
```

implement → gate → repair → escalate → review → validate, stopping whenever
something needs judgement (exit 3: gate still failing after escalation; exit 4:
exit criteria unmet). Phase 7 — read the diff, commit — is never run for you.

---

## The skills, and how they chain

```
sdd-orchestrate                        (Claude — the master loop)
   │
   ├─ phase 1 ─→ spec-author           (Claude — write the contract)
   │
   ├─ phase 2-6 ─→ pi-delegate         (Claude — how to drive pi)
   │                  │
   │                  ├─ phase 2 ─→ implementer     (pi — TDD, writes code)
   │                  ├─ phase 3 ─→ lib/quality-gate.sh        (script, no model)
   │                  ├─ phase 4 ─→ implementer     (pi — repair, same session)
   │                  ├─ phase 5 ─→ reviewer        (pi — READ-ONLY review)
   │                  └─ phase 6 ─→ lib/validate-exit-criteria.sh (script, no model)
   │
   ├─ phase 7 ─→ verify-and-commit     (Claude — reads diff, commits)
   │
   └─ phase 8 ─→ handoff check         (shell — HANDOFF.md fresh?)
```

| skill | layer | phase | role |
|---|---|---|---|
| `sdd-orchestrate` | Claude | all | entry point; calls everything below |
| `spec-author` | Claude | 1 | write a contract a cheap model can implement without judgement |
| `pi-delegate` | Claude | 2–6 | tool grants, sessions, escalation, token discipline |
| `verify-and-commit` | Claude | 7 | read the load-bearing diff, confirm architecture, commit |
| `implementer` | pi | 2, 4 | tests first, then code, until the gate passes |
| `reviewer` | pi | 5 | read-only review against the spec |

Plus:

- `pi/DIRECTIVES.md` — operating rules, passed to pi on **every** call
- `lib/quality-gate.sh` — phase 3. Detects Rust / Node / Python / Go and runs
  format, typecheck, lint and test. A project may override with
  `deploy/scripts/quality-gate.sh`, but is never required to have one.
- `lib/validate-exit-criteria.sh` — phase 6. Parses `- [ ]` items from the
  spec's Exit Criteria and runs each backticked command.

**Skills own their helpers.** `spec-author` carries `templates/spec-template.md`
and `scripts/new-spec.sh`; `verify-and-commit` carries the HANDOFF template and
`scripts/new-handoff.sh`. A skill installed without its scripts is the same
not-self-contained defect one level down, so the installer copies them together.

**Why `lib/` exists rather than folding those into a skill.** The installer puts
Claude skills under `~/.claude/skills` and pi skills under `~/.pi/agent/skills` —
two separate trees. `quality-gate.sh` has consumers in both: the harness runs it
at phase 3, and pi runs it itself while iterating at phases 2 and 4. Placing it
in one skill's `scripts/` would force the other tree to reach across into it.
`lib/` is the shared layer; a script used by exactly one skill belongs in that
skill's `scripts/`, and both of those do.

**They are scripts, not skills.** `quality-gate` and `validate-exit-criteria`
take no model and make no decisions. Two skill bodies originally called them
"skills", and `doctor` whitelisted those two names so the check would pass — a
check softened to accommodate a defect keeps the defect invisible. The whitelist
is gone: a name written as a skill must resolve to an installed skill.

---

## The orchestrator is replaceable

The orchestrator-side skills are plain Markdown invoking shell commands. Neither
the harness, the libraries, nor `DIRECTIVES.md` has any functional dependency on
Claude — the only such strings are the default skills directory path, and
`install.sh --orchestrator-dir` relocates that.

Any agent that reads instructions and runs shell commands can drive this. The
orchestrator's model and the workhorse's model are independent choices: the
first plans, judges and commits; the second implements.

## Why a directives file rather than `CLAUDE.md`

`pi` auto-loads `AGENTS.md` / `CLAUDE.md` from its working directory, and does
**not** auto-load `~/.pi/agent/system.md` — that arrives only through an
interactive shell alias, which does not expand in a non-interactive shell. A
programmatic call therefore gets **no operating rules at all** unless you pass
them.

Relying on the auto-load would be wrong anyway: `CLAUDE.md` carries *solution*
detail — what a particular project is — and on a new machine may be absent,
stale, or written for another audience.

So the rules live in `pi/DIRECTIVES.md` and are passed explicitly every time.
**Rules travel with the harness; solution detail stays with the project.**

---

## Design decisions worth keeping

**Roles are separated by tool grant, not by instruction.** The reviewer gets
`read,grep,find,ls` and structurally cannot edit. A reviewer that cannot write is
worth more than a reviewer told not to.

**`gate`, `validate` and `handoff` invoke no model.** They are exit codes.
Opinion does not enter.

**No model is hardcoded.** Omitting `--model` uses pi's own configuration;
pinning one would silently contradict it. Escalation is one setting in
`config.env`.

**Phase 7 is never delegated.** A workhorse may never commit. Reading the
load-bearing diff is the judgement the cheaper model was not asked to supply.

**Structure is generated, not written.** `new-spec.sh` emits the six sections
from the template so every spec has the same shape; the model fills content
only. Spec shape stops depending on how well a template was paraphrased that
day.

**A criterion's command must exit 0 when satisfied.** A check that should find
nothing is written `! grep -q pattern path` — a bare `grep -q` exits 1 on
success and reports a false failure. Three of the first nine real criteria run
through the validator were wrong this way.

---

## Configuration — `config.env`

| setting | default | meaning |
|---|---|---|
| `ORCH_MODEL` | *(empty)* | normal work; empty means pi's own config. Leave empty. |
| `ORCH_ESCALATION_MODEL` | `glm-5.2:cloud` | used with `--escalate` |
| `ORCH_ESCALATION_PROVIDER` | `ollama-cloud` | provider for the above |
| `ORCH_MAX_REPAIR_ROUNDS` | `2` | repair attempts before escalating |
| `ORCH_MAX_TOOL_ITERATIONS` | `60` | bound on the agent's tool loop |
| `ORCH_REQUEST_TIMEOUT` | `600` | seconds per turn |

All overridable by environment variable of the same name.

---

## Enforced conventions

**`specs/`** — every non-trivial change starts with a spec. pi may read them and
may never edit them: a worker that can edit the spec can make any failure vanish
by rewriting the requirement.

**`HANDOFF.md`** — the implementation diary, checked by `pi-workhorse.sh
handoff`. Read before starting, updated before finishing.

**`_tmp/`** — all scratch and test artefacts. Never `/tmp/`, never an absolute
path: those pass locally and fail on a fresh checkout.

---

## Measured, not asserted

Three specs run end to end against a real crate, project-local install
(`deepseek-v4-flash` implementing, `glm-5.2:cloud` on escalation):

| spec | outcome | calls | billed tokens |
|---|---|---|---|
| 01 parser, clean-room | gate passed first round, 7/7 criteria | 20 | 214,638 |
| 02 **false premise** | **refused, wrote no code** | 15 | 156,152 |
| 03 formatting, `u64` trap | gate passed first round, 7/7 criteria | 16 | 167,472 |
| escalation, injected defect | found and reverted it | 15 | 179,935 |

`cacheRead` is most of each figure and is normally discounted, so "billed" is
an upper bound rather than a cost.

**Spec-02 is the result that matters.** Its premise was false — it claimed a
struct that does not exist. The workhorse checked the code, refused to invent
the type, cited the guardrail by name, and rejected the tempting workaround on
the grounds that *"decomposing u64 seconds to feed a made-up struct would pass a
green gate while the stated API composition remains impossible."* It identified
that the workaround would produce a false green. That cost 156k tokens and was
worth every one.

**The escalation test.** Injecting `(total_seconds / 3600) as u32` into a
finished implementation gives **20 passed, 1 failed** — only the `u64::MAX`
round-trip catches it. That is this entire design in one line: a suite that
looks green, one carefully chosen case that does not. `glm-5.2:cloud` found it,
and noticed the tree had changed since its own earlier review.

## What the directives encode

They are not generic hygiene. Each rule is a defect that shipped somewhere:

- a compatibility guard **regenerated by the very commit it was guarding**, so it
  could not fail
- a third of every spec silently dropped by a nine-line parser, whose tests used
  fixtures while production used real files
- a budget breach that marked the run complete, so resume refused to continue it
- five requirements withdrawn because a worker **reported a bad premise** instead
  of coding around it — the most valuable output those runs produced

The recurring shape is **a green suite beside a broken feature**, because the
check exercised the unit and never the seam. Hence: assert on what the consumer
received, verify by running the gate, and read the load-bearing diff anyway.
