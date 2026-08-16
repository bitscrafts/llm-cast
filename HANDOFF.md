# HANDOFF — chromecast-tv-mirror

**Status**: YELLOW — spec-02 damage tracker implemented and test-verified in
isolation, but quality gate cannot pass: `alacritty_terminal 0.24.2` does not
compile on this toolchain (upstream defect, see below). SPEC-DEFECT reported.
**Date**: 2026-08-16
**Phase**: 3 — spec-02 terminal cell damage tracker

---

## 2026-08-16 — spec-02 damage tracker: code done, gate blocked by dependency defect

### What was DONE this session
- **`src/lib.rs`** (NEW): declares `pub mod damage;`.
- **`src/damage.rs`** (NEW): `CellKey {row: i32, col: usize}` (Debug/Clone/Copy/
  PartialEq/Eq/Hash), `CellContent {ch, fg, bg, flags}` (Debug/Clone/Copy/
  PartialEq/Eq), `DamageTracker::new()` / `diff(&[(CellKey, CellContent)]) -> Vec<CellKey>`
  / `reset()`, plus `Default`. Semantics: first call damages everything; identical
  input → empty; removed keys forgotten (reappearance = first-time damage, D5);
  output sorted row desc / col asc (D6); duplicate keys per call last-write-wins,
  reported at most once; no unwrap/expect/panic (N2).
- **`tests/cast_tv_tests.rs`** (NEW): all 10 TDD-Contract tests (D3, D4, D2a/b/c,
  D5a, D5b acceptance, D6, D7 + duplicate-key). Wrote tests FIRST; they caught a
  real bug in my first tracker draft (unchanged keys were purged from the retained
  map because `seen.insert` only ran in the changed-branch — fixed).
- **`Cargo.toml` and `src/render/font.rs` untouched** (N1/N4). `cargo fmt` clean on
  new files.

### Outcome
- Tracker verified CORRECT: isolated scratch crate `_tmp/damage-verify/` (no
  alacritty dep; same source + tests) → `test result: ok. 10 passed; 0 failed`.
- **Gate `QUALITY GATE: FAILED (rust)`, exit 1** — fmt PASS, but check/clippy/test
  FAIL, all with the SAME root cause (9× E0277/E0308, all in
  `alacritty_terminal-0.24.2/src/tty/unix.rs`):
  `alacritty_terminal 0.24.2` → `rustix-openpty 0.1.1` (a `#![no_std]` crate) →
  `rustix 0.38.44` with features alloc/fs/termios but **NOT std**; nothing else in
  the graph depends on rustix 0.38.x, so its `std` feature can never be enabled →
  rustix uses its no_std fd polyfill whose `AsFd` trait differs from
  `std::os::fd::AsFd` → `tcgetattr`/`tcsetattr` calls in alacritty_terminal fail to
  compile. Upstream defect (known; fixed only in rustix-openpty 0.2.0 /
  alacritty_terminal 0.25+, both UNREACHABLE under `alacritty_terminal = "0.24"`
  / `rustix-openpty = "0.1.1"` without editing Cargo.toml). This was latent at
  baseline: the missing-test-file resolution error aborted before dependency
  compilation, so it was never observed.
- **SPEC-DEFECT reported**: exit criteria (gate passes) unreachable under G3/N1
  (Cargo.toml untouchable). This is a spec premise error, not a code error.

### What REMAINS (next agent — resume here)
1. Fix the dependency before the gate can ever pass:
   - Edit `Cargo.toml`: either add `rustix = { version = "0.38", features = ["std"] }`
     (unblocks rustix 0.38.44's std polyfill), or bump `alacritty_terminal` to
     0.25/0.26 (which uses rustix-openpty 0.2 → rustix 1.x). Requires a spec
     amendment (G3 forbids it as-is).
   - A lock-only `cargo update` CANNOT fix it (verified: 0.24.2 is the last 0.24.x;
     rustix-openpty 0.2.0 needs rustix ^1.0 and is not allowed by 0.24.2's req).
2. Then re-run the gate; spec-02 code should need no further changes.
3. After spec-02 lands: renderer diff consumption, then spec-01 modules (emu,
   render, capture, serve, cast, encode) per `specs/01-cast-tv-terminal.md`.

## Previous session (2026-08-10) — retained below

## What was DONE this session

- Verified environment: rustc/cargo 1.95.0, GStreamer 1.26.2 dev libs present,
  Cargo.lock pins alacritty_terminal 0.24.2 / rust_cast 0.17.0 / tiny-skia 0.11.4 /
  vte 0.13.1 / gstreamer 0.22.8. `cargo fetch` OK (CARGO_HOME=/usr/local/cargo).
- **API intel verified by reading crate sources** → `docs/02-research/api-intel-v1.md`
  (authoritative for the next agent — read it first!). Key findings:
  - alacritty_terminal 0.24.2: `Term::new(config, &(cols,rows), VoidListener)`,
    `Term` impls `vte::ansi::Handler`; feed via `vte::ansi::Processor::advance(term, byte)`.
    Grid: `grid[Line(i32)][Column(usize)]`, visible rows `Line(0)`..`Line(-(rows-1))` (bottom = -1).
    `Cell{c,fg,bg,flags}`; `Color::Named|Spec|Indexed`; `Flags::BOLD|DIM|ITALIC`.
    **No public damage API in 0.24.2 → own diff via HashMap of last cells.**
  - tiny-skia 0.11.4: `Pixmap::new`, `fill`, `data()` = premultiplied RGBA.
  - rust_cast 0.17.0: `CastDevice::connect(host,port)`; discovery = mdns (needs
    timeouts to satisfy `test_sender_reports_unreachable`).
  - **No H.264/HLS GStreamer plugins in container** (no vaapi/bad/ugly) → encode
    module must be feature-gated plan-only in default build (`gstreamer` feature).
- Memory stored: `chromecast-tv-mirror/implementation/api-intel-v1` (importance 0.8).
- Font staged: `src/font8x8_basic.h` (public-domain 8×8 bitmap font, 152 lines,
  from /tmp download; rasterizer should embed it).

## What REMAINS (next agent — resume here)

1. Write `src/emu/` (term wrapper + ScreenFrame/Cell diff), `src/render/` (raster),
   `src/capture/` (bridge), `src/serve/` (axum HLS + CORS), `src/cast/` (sender),
   `src/encode/` (gated pipe), `src/lib.rs`, `tests/cast_tv_tests.rs` — full plan in
   `specs/01-cast-tv-terminal.md` (module tree, data structs, 10 tests).
2. Then run the TDD cycle: `cargo test --test cast_tv_tests` (guardrail: cargo only
   inside the TDD cycle; fix → retest → verify exit criteria → quality-gate).
3. Exit criteria checklist at bottom of spec; `tests/cast_tv_tests.rs` already wired
   in Cargo.toml `[[test]]`.
4. Milestone-1 device smoke test (operator): rust_cast media_load an HLS URL.

## Key files
- `docs/02-research/api-intel-v1.md` — VERIFIED API reference (read first)
- `src/font8x8_basic.h` — staged font for rasterizer
- `specs/01-cast-tv-terminal.md` — the spec (TDD contract, exit criteria)
- `Cargo.toml` — deps already approved/declared; do NOT add without approval

## Notes / gotchas
- **Not a git repo** (no .git) — consider `git init` + commit at end of next session.
- GStreamer plugins (vaapih264enc/hlsmux) are target-host runtime deps; encode module
  compiles without them (feature-gated).
- Memory topic `chromecast-tv-mirror`; session rule: search before acting, store at
  task end (see AGENTS.md MANDATORY MEMORY PROTOCOL).

## Memory keys
- `chromecast-tv-mirror/implementation/api-intel-v1` (NEW this session)

---

## PIDAG RUN RESULTS (2026-08-10, run ed3bc4990bcd) — post-upgrade live test

**Outcome**: DAG COMPLETED — `successful_nodes:2, failed_nodes:2`. Worker `run`
process ended; `sdd` driver (PID 617491) still idle.

### Node-by-node
1. `validate-baseline` ✗ failed (nothing implemented yet — expected).
2. `implement-iter1` ran as `pi -p --mode json --model deepseek-v4-flash` (REAL worker,
   compaction fixed, full JSON-lines agent transcript). **Wrote ZERO source files** —
   it re-verified already-documented APIs, hit ITS OWN tool-iteration budget, and
   emitted a handoff envelope instead of code. It overwrote `HANDOFF.md` (restored via git).
3. `quality-gate-1` → `passed:true` but `fmt:false` + `test:false` (missing
   tests/cast_tv_tests.rs). **BUG**: quality-gate masks failures with `|| echo ...passed:true`
   in fmt/clippy/test branches → `passed` can be `true` even when tests fail.
4. `validate-iter1` ✗ failed (exit criteria unmet).
5. `implement-iter2` ⛔ **`NodeBlocked`** (deps unsatisfied because validate-iter1 failed).
6. `DagDone`.

### Key pidag findings (for pidag dev)
- **`works` end-to-end post-upgrade** (worker dispatches, runs, returns, gates run).
- **Bug A**: SDD loop does NOT feed a failed validation back into iterate — implement-iterN+1
  gets BLOCKED on a failed validate-iterN instead of being given the failure to fix. So a
  single failed iteration terminates the loop (no self-healing). Likely in sdd DAG gate logic.
- **Bug B**: quality-gate `passed:true` despite failing fmt+test (the `2>/dev/null || echo passed:true`
  fallbacks mask real failures). quality-gate should NOT swallow cargo fmt/test failures.
- **Bug C (worker quality)**: a single `implement-iter1` "from scratch" node is too big; worker
  re-derives API intel and exhausts its turn budget before writing files. Consider: pass api-intel
  doc path in the implement prompt, or split into per-module implement nodes.
- **No 429/exhaustion occurred** (deepseek-v4-flash answered every call), so free[0]→free[1]
  fallback and iter3 paid-escalation were NOT exercised in this run. To test exhaustion
  deterministically, use `TypeDispatchWorker::with_pi_command` + a fake `pi` that emits 429
  (see /opt/pidag-src/src/scheduler/execute.rs + src/worker/mod.rs), or a real 429 from the API.

### Next for pidag (parts)
- Fix Bug A (validation-failure → pass failure text to next implement iter) and Bug B
  (quality-gate honesty). Re-run the DAG to confirm the worker then converges.
- Re-run `pidag sdd specs/01-cast-tv-terminal.md --run` after pidag fixes; worker may then write files.
