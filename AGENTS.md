# AGENTS.md — chromecast-tv-mirror (Research Project)

Research project to define a solution that displays an external terminal
multiplexer session ("outer/herd") on a TV via a Google Chromecast.

**Key point**: This is a RESEARCH phase. There is no implementation code yet —
the working artifact is `docs/` (research findings, options, trade-offs) plus a
proposed architecture. Follow research conventions, not SDD-code conventions,
unless/until an implementation spec is approved.

**Pi is the agent. All Python via UV.**

---

## MANDATORY MEMORY PROTOCOL (do not skip)

The agent-memory backend (`http://10.10.10.217:7420`) is the single source of
truth for accumulated insights. Use it on EVERY research step and at EVERY task
end. Helpers:

```bash
MEM=/root/.pi/agent/skills/agent-memory/run.sh
$MEM health                          # verify service is up before working
$MEM search <topic> <query> [k]      # RETRIEVE before acting
$MEM store <topic> <key> <content> [importance]      # STORE a finding
$MEM store-global <key> <content> [importance]       # cross-topic insight
```

### Rules (enforced)

1. **Search before you act.** Before researching a topic, do
   `$MEM search chromecast-tv-mirror "<topic query>"`. If a prior insight exists,
   reuse/refine it — do NOT re-derive known results.
2. **Store at every task end.** Any task that produces a finding, a decision,
   a benchmark, or new research MUST call `$MEM store ...` and the store MUST
   return a non-empty `key` before the task counts as complete.
   A task is NOT done until its insight is persisted.
3. **Topic = `chromecast-tv-mirror`** for all project insights.
   Use stable keys: `<project>/<category>/<identifier>` (e.g.
   `chromecast-tv-mirror/architecture/cast-stream-pipeline`).
4. **Importance scoring**:
   - 0.9 critical/blocking finding
   - 0.8 architectural decision / chosen option
   - 0.7 implementation finding
   - 0.5 research observation / experiment
   - <0.4 uncertain (decays)
5. **Search before store** — dedupe: if a nearly-identical insight already
   exists with the same key, update it rather than duplicating.
6. At the end of a session, also summarize the memory keys used in `HANDOFF.md`.

### Memory cheat-sheet commands
```bash
bash /root/.pi/agent/skills/agent-memory/run.sh health
bash /root/.pi/agent/skills/agent-memory/run.sh search chromecast-tv-mirror "chromecast terminal display" 5
bash /root/.pi/agent/skills/agent-memory/run.sh store chromecast-tv-mirror architecture/cast-pipeline "fnd: ..." 0.8
```

---

## RESEARCH WORKFLOW (doc-organizer maintained)

The skill `doc-organizer` organises `docs/` with SSOT methodology:

```bash
DOC=/root/.pi/agent/skills/doc-organizer/run.sh
$DOC discover   /projects/chromecast-tv-mirror
$DOC analyze    /projects/chromecast-tv-mirror   # find dupes/overlaps
$DOC organize   /projects/chromecast-tv-mirror   # build docs/ hierarchy
$DOC index      /projects/chromecast-tv-mirror   # master index + TOC
```

Per research task:
1. `agent-memory search` for prior insights on the subtopic.
2. Gather facts from the internet (Chromecast, cast protocol, terminal → video,
   GPU/VA-API encoders, WebRTC/MJPEG/HLS, etc.).
3. Write ONE updated markdown note under `docs/` (see layout). Keep each fact in
   exactly one place (SSOT).
4. Update `docs/INDEX.md` if a new doc is added.
5. `agent-memory store` the distilled insight (importance ≥ 0.5).
6. Re-run `$DOC index` so the TOC stays current.

## docs/ layout (SSOT hierarchy)

```
docs/
├── INDEX.md                      # master index (keep updated)
├── 01-overview/
│   └── problem-statement.md      # the goal + constraints
├── 02-research/
│   ├── chromecast-capabilities.md
│   ├── cast-for-terminal.md      # how to push a terminal/stream to cast
│   ├── streaming-options.md      # MJPEG / WebRTC / HLS / RTSP / VNC
│   ├── encoding.md               # ffmpeg, VA-API, h264/hevc/av1
│   └── muxer-client.md           # what "outer/herd" provides (tmux?)
├── 03-architecture/
│   └── proposed-options.md       # candidate solution architectures
└── 99-decisions/
    └── ADR-0001-<decision>.md    # architecture decision records
```

## Directory structure

```
/projects/chromecast-tv-mirror/
├── .pidag/        # pidag run history
├── specs/         # (only when an implementation spec is approved)
├── docs/          # research notes (doc-organizer managed)   <— working artifact
├── src/           # (only after implementation begins)
├── AGENTS.md
└── HANDOFF.md
```

## Google Chromecast research focus areas (start here)

1. **Cast protocol / Cast v2**: how a sender pushes a receiver (app) to the
   device, media streaming (MSE), the default "media receiver" experience.
2. **How to show a live/terminal feed**: Chromecast is NOT a browser you control;
   it runs a Google Cast app. Options: cast a web page that shows the terminal,
   or use a media stream (HLS/MJPEG/WebRTC) rendered in a receiver app.
3. **Transcoding**: terminal → video must be encoded (likely ffmpeg with VA-API
   hardware encode on the host). Evaluate latency + CPU cost.
4. **Feasibility of alternatives**: a cheap HDMI dongle/display server
   (e.g. a Raspberry Pi with a screen share), vs Chromecast-specific receiver
   (requires registering with Google / Cast SDK), vs DiRAC/virtual-cast.
5. **"outer/herd"** terminal multiplexer: confirm it is a client/server tmux-like
   tool and what remote-attach/SSH display it already supports you can lean on.

Trail items below (leave details to docs/ + memory).

## Handoff

After each session update `HANDOFF.md` via:
```bash
/root/.pi/agent/skills/handoff-generator/run.sh /projects/chromecast-tv-mirror
```
Include: status (GREEN/YELLOW/RED), what was researched, decisions, next
research steps, and the memory keys stored.

---

# EXPERIMENT LOG (IMPORTANT — READ FIRST)

This project doubles as a **robustness experiment** on pidag + pi-in-a-pipeline.
Goal: check pidag SDD robustness and how faithfully an agent follows a pipeline.
**Log every detail and every problem here, even the tooling problems we hit
while building/builders of the tooling.** Each entry: date, what, symptom, root
cause (if known), impact, and the follow-up to attack later. Keep appending;
never delete history. Mirror these to agent-memory at task end (see MEMORY).

## 2026-08-07 — Tooling / workflow problems observed (experiment run)

### P1 — Repeated "plan-to-act" narration loop (pi behavior), HIGH impact
- **Symptom**: Multiple times the agent emitted a dozen+ back-to-back turns of
  "let me do X" without actually invoking the tool; a tool call was finally made
  only after the user said "proceed". Cost several minutes + token budget per loop.
- **Root cause (hypothesis)**: the model can drift into re-stating an already
  decided micro-action each turn rather than acting; no hard self-enforcement that
  "stated action → tool call in the same block". Some of this was ME (the agent)
  in this very session.
- **Impact**: wasted turns/tokens; frustrates a pipeline/cron use case; tests the
  "pi follows a pipeline" goal negatively.
- **Fix to attack later**: add a rule to AGENTS.md (done — see MEMORY PROTOCOL)
  + possibly prompt/tool-behavior enforcement in pi: when a tool is required,
  emit it in the same block as the plan; detect >N consecutive "plan-only" turns
  and force an action. Logged to memory as `pi-usage/failure-pattern/repeated-planning`.

### P2 — Memory store endpoint non-obvious, HIGH impact (first boot only)
- **Symptom**: `agent-memory store` returned empty; a guessed `POST /store` 404'd.
- **Root cause**: correct route is `POST /v1/memory/{topic}/insights` with a
  `scope:{type:topic,...}` body (run.sh knows it); `/health` is also a 404 (the
  real health probe is a search call). The skill's SKILL.md is vague about the
  wire format.
- **Impact**: first stores silently missed; requires endpoint knowledge.
- **Fix to attack later**: document the exact `/v1/memory/{topic}/insights`
  contract in agent-memory SKILL.md; make `store` print a clear non-empty
  acknowledgment (it does, `{"key":...}`) — verify stores by checking that output.

### P3 — doc-organizer rewrites INDEX.md (SSOT drift), MEDIUM impact
- **Symptom**: my hand-maintained `docs/INDEX.md` was overwritten by
  `doc-organizer index` into a minimal auto-format (Sections + README hooks) and
  `TOC.md` becomes the real index.
- **Impact**: don't hand-edit INDEX.md under an SSOT tree; let doc-organizer own
  it and rely on TOC.md. (Noted so the next agent doesn't fight the tool.)

### P4 — Web scraping fragility, MEDIUM impact
- **Symptom**: Wikipedia 403/bot-block intermittently; Google HTML search returns
  no links; DuckDuckGo rate-limits (empty) after several queries; some Google
  `developers.google.com/cast/docs/*` paths 404 (renamed).
- **Fix to attack later**: prefer direct authoritative fetches + a polite UA; add
  retry/backoff and a caching layer for research fetches; keep a sources ledger.

### P5 — GitHub API sort-parameter gotcha, LOW impact
- **Symptom**: `forks?sort=updated` → 400; `sort=stargazers` with `per_page=6`
  returned `[]` (repo genuinely has 0 forks — a useful finding, but the 400 was
  an API-contract trap).
- **Fix**: use documented sort values; treat empty forks as a real signal (done).

### P6 — Tool-call narration loop tripped by `python3 -c` multi-line quotes, LOW impact
- **Symptom**: embeddings commands with `break` outside a loop and multi-line
  `python3 -c` broke; also repeated identical narration around a single bash call.
- **Fix to attack later**: use heredocs (`python3 - <<EOF`) for multi-line snippets,
  and one tool call per decided action (see P1).

## 2026-08-07 — pidag robustness observations (will deepen after --run)
- `pidag sdd specs/01-...md` correctly parsed the numbered spec and generated the
  DAG node graph (impl + quality-gate + validate). 
- `pidag sdd --help` is NOT accepted (bare `--help` rejected as a bad spec name) —
  CLI help ergonomics worth logging; docs say use `pidag sdd <spec>`.
- `pidag split`, `pidag auto --lock`, queue/weight/resume/pidlock all worked in the
  pidag project tests (see /projects/pidag HANDOFF + git baseline f9c4928).
- Expect to log scheduler/runner behavior, quality-gate interplay, and exit-cc
  re-validation once `pidag sdd --run` executes.

### How to keep this log
- Append new dated entries here after every session / experiment step.
- Mirror each to agent-memory (`store-global pi-usage/...` or `store project/...`)
  BEFORE declaring task complete, then roll the keys into HANDOFF.


## 2026-08-07 — SDD run failure analysis (pidag robustness, MILESTONE)

### Run: run-20260807-045349-d300bb · `pidag sdd specs/01-cast-tv-terminal.md --run`
- DAG: 10 nodes generated; concurrency=4, allow_paid=false.
- Result: `0/10 done · 2 failed`; correct dependency blocking held
  (quality-gate-1 → Blocked; later iters → Pending). **Pipelining worked.**
- **P7 (config gap)**: project `.pidag/config.toml` was NOT created by
  `pidag attach` (empty/absent; earlier output claimed "Initialized"). We ran
  with defaults (free-tier only, default model). Log: verify attach writes config.
- **P8 (wrong-model dispatch / planner-vs-worker)**: `implement-iter1` was
  dispatched with `nvidia/z-ai/glm-5.2` — a PLANNER model — not a worker
  (expected `deepseek-ai/deepseek-v3.2` or gemini-2.5-flash, or paid
  `deepseek-chat`). It hit `429 backoff` → retry attempt 2 → "execution failed",
  **without falling back to the paid DeepSeek** chain. The free→paid fallback
  did NOT engage after repeated 429. Attack later: align implement-node default
  model to the worker list and ensure N×429 → fallback to next model tier.
- **P9 (blocking baseline gate)**: `validate-baseline` is a hard gate that runs
  the EXIT-CRITERIA validator pre-implementation; since the spec's criteria
  reference not-yet-existing files (`src/lib.rs`, `src/capture/bridge.rs`, …) it
  fails and blocks the whole DAG. Recommend: baseline gate should only CHECK the
  criteria PARSE (or be a pre-impl gateway that expects "not implemented" and
  does not hard-block), OR exit-criteria file checks should tolerate absence
  until first implement-iter. This is a spec/DAG-policy design point.
- Follow-up: rerun once with a project config that pins worker models + paid
  fallback, and a baseline gate that doesn't hard-fail pre-impl; log result.

## 2026-08-07 — NVIDIA model naming discovery (tooling debug, IMPORTANT)
- Checked the authoritative NVIDIA NIM LLM API reference
  (https://docs.api.nvidia.com/nim/reference/llm-apis, endpoint
  https://integrate.api.nvidia.com POST /v1/chat/completions). It lists these
  model id paths: `deepseek-ai/deepseek-v4-flash`, `deepseek-ai/deepseek-v4-pro`,
  `z-ai/glm-5.2`, `google/*`, etc. — i.e., the **full provider/model path**.
- **P12 (provider routing)**: In THIS pi install, `pi -p --model nvidia/ANYTHING`
  does NOT route to NVIDIA — everything is routed to the **deepseek** provider
  (settings.json provider=deepseek). The deepseek endpoint rejects any string
  except the two model names it natively supports:
    'deepseek-v4-pro' | 'deepseek-v4-flash'.
  Verified: `nvidia/…`, `google/gemini-2.5-flash`, `deepseek:deepseek-chat` all
  400; only `deepseek-v4-flash`, `deepseek-v4-pro`, `deepseek-chat` -> PONG.
- **Conclusion / config corrected**: this environment's reachable workers are
  exactly `deepseek-v4-flash` (free #1) + `deepseek-v4-pro` (free #2) + paid
  `deepseek-chat`. The project config is now set accordingly (verified earlier).
- **Gap to attack later**: to reach "google gemini flash" or a true NVIDIA/GLM
  fallback we must add/verify those providers in settings.json/models.json and
  wire pi's model-name->provider routing (it currently ignores `provider/` prefixes).
  Not done in this session (infra change, out of scope for the SDD run).

## 2026-08-07 — SDD re-run outcome (config fix verified; pidag issues remain)
### Re-run: run-20260807-051252-a82b7d (after fixing .pidag/config.toml)
- **CONFIG FIX CONFIRMED**: `implement-iter1` now dispatches with
  `deepseek-v4-flash` (correct worker model) and, on failure, performs
  `ProviderFallback deepseek-v4-flash → deepseek-v4-pro` — the free-model
  exhaustion/fallback chain (P8) now WORKS with two distinct free models.
- **Still failed** after both free models: `implement-iter1` -> "attempt failed"
  x2 -> "execution failed". So the model routing is fixed; the *content*
  execution of the implement LLM node still fails (need to capture the LLM's
  actual output/exit-cc — the run log is terse; investigate pidag node
  execution error surfacing).
- **validate-baseline still hard-fails** and blocks the DAG (P9 unchanged):
  runs the exit-criteria validator pre-implementation against not-yet-existing
  files. This is the start-to-end blocker the user wants fixed.
### Remaining pidag-debug targets (the actual work now)
- D1. Implement-node "execution failed" root cause: capture + surface the LLM
  node's real output/error (not just "execution failed"). Possibly the piped
  exit-criteria or quality-gate inside the node, or a validator call that
  returns non-zero, or node output/pipe handling.
- D2. `validate-baseline` must not hard-block pre-implementation: make baseline
  parse-only, or skip criteria that reference not-yet-created files, or treat
  it as an informational gate.
- D3. (logging) Make `pidag show`/sdd-run log emit node failure detail by default
  so these are debuggable without guessing.

## 2026-08-07 — Provider routing fixed: nvidia->exhaust->GOOGLE gemini (IMPORTANT)
- Authoritative model IDs confirmed:
  - Gemini 3.6 Flash = `gemini-3.6-flash` (workhorse; from the Gemini 3 catalog),
    google provider.
  - glm planner = `z-ai/glm-5.2`, nvidia provider.
- VERIFIED via direct pi PONG tests:
  - `pi -p --provider google --model gemini-3.6-flash` -> PONG (REACHABLE)
  - `pi -p --provider nvidia --model z-ai/glm-5.2`  -> PONG (REACHABLE)
  - `pi -p --model <anything>` w/o --provider -> routes to deepseek (PI_PROVIDER=deepseek)
- **P13 (routing root cause)**: `PI_PROVIDER=deepseek` env + the fact that
  pidag's worker invokes `pi --model <string>` WITHOUT `--provider` means a
  `google/...` or `nvidia/...` model string STILL goes to deepseek and 400s.
  To make `on-nvidia-exhaust->google gemini` work, the model->provider mapping
  MUST pass `--provider` (or otherwise select the provider), not rely on the
  model-string prefix.
- FIX PATH (next): teach the pidag worker to emit `--provider <provider>` given
  a `provider/model` in the config [models] strings, OR add an explicit
  provider field; then `free = [nvidia deepseek-v4-flash, google/gemini-3.6-flash]`
  gives exhaustion fallback across DIFFERENT providers (what the user wants),
  and `planners` = glm-5.2 (nvidia). This is pidag-project work (debug pidag).
