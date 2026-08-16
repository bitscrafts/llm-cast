# HANDOFF — chromecast-tv-mirror

**Status**: GREEN — spec-01 **all 4 parts done** (part 1 emu+render, part 2
capture+cast, part 3 serve+encode, part 4 closing sweep). Gate GREEN, all 21
tests pass, all 4 part-4 exit criteria pass. The 10-test parent TDD contract
is fully present and passing.
**Date**: 2026-08-16
**Phase**: 6 — spec-01 complete; next: optional rust_cast/gstreamer feature
integration (media_load onto device, encoder feed), or ADR on option 2

---

## 2026-08-16 (part 4 session) — closing sweep; gate GREEN

### What was DONE this session
- **TDD first**: appended the missing 10th contract test
  `test_no_production_unwrap` (R8) to `tests/cast_tv_tests.rs` — walks the six
  module dirs (`src/capture|emu|render|encode|serve|cast`) relative to
  `CARGO_MANIFEST_DIR` and fails on any non-test, non-comment line containing
  `.unwrap()`/`.expect()`. Also asserts the walk is NOT vacuous: all six dirs
  must exist and have been walked (a missing dir fails the test rather than
  passing silently). Cleaned up the unused `Path` import.
- **No production code changes needed**: the existing six module files already
  comply with R8 (grep for `\.unwrap()|\.expect(` in src/ finds only
  `unwrap_or` in emu/term.rs, which is not a call to unwrap()).
- **Suite**: 21/21 pass (`test result: ok. 21 passed; 0 failed; ...`),
  including the new `test_no_production_unwrap ... ok`. The full parent TDD
  contract (all 10 tests) is present and green.

### Outcome — gate GREEN (exit 0)
```
  cargo fmt --check            PASS
  cargo check                  PASS
  cargo clippy -D warnings     PASS
  cargo test                   PASS
QUALITY GATE: PASSED (rust)
```

### Exit criteria — all 4 pass (EC1-EC4 all exit 0)
- EC1 `cargo test --test cast_tv_tests` → "test result: ok"
- EC2 all six module files exist
- EC3 wire field `"type": "LOAD"` in src/cast/sender.rs (the orchestrator
  amended this criterion after the run — the old `grep "media/load"` passed
  via doc comments only; same fix as part 2)
- EC4 all six module dirs exist AND no `.unwrap()`/`.expect()` in them

### Memory keys stored this session
- `chromecast-tv-mirror/implementation/part4-no-unwrap-sweep` (0.7) — part-4
  sweep done: 10/10 parent contract tests present+passing, no production
  unwrap/expect; test asserts walk non-vacuous.

### Next steps
- Optional (feature-gated, needs system deps): rust_cast `media_load` HLS onto
  a real device (cast feature) + encoder output feed into the HLS server
  (gstreamer feature). If rust_cast cannot media_load HLS, stop and report for
  explicit Option-2 decision (pivot guardrail).
- Consider ADR for option 2 (HDMI dongle vs Chromecast) once live test happens.


---

## 2026-08-16 (part 3 session) — serve + encode implemented; gate GREEN

### What was DONE this session
- **TDD first**: appended the two part-3 contract tests (parent T5/T6) to
  `tests/cast_tv_tests.rs` — `test_hls_playlist_has_cors`,
  `test_served_segment_bytes`. Raw HTTP/1.1 GET over a `TcpStream`
  (`Connection: close`, `Origin` header) stands in for the receiver's fetch —
  no client dependency; server spawned via `TcpListener::bind("127.0.0.1:0")`
  (bound before spawn ⇒ no race) + `axum::serve`.
- **`src/serve/mod.rs`** (NEW): `pub mod server;`
- **`src/serve/server.rs`** (NEW, R5): axum 0.7 router — GET `/live.m3u8`
  (playlist const, `application/vnd.apple.mpegurl`) and GET `/segment/:name`
  (static blob `SEGMENT_BYTES`, 404 for unknown names); CORS via
  `CorsLayer::new().allow_origin(AllowOrigin::any())`; pub consts `PLAYLIST`,
  `SEGMENT_BYTES`, `CORS_ALLOW_ORIGIN`; handlers return `Response`, never
  panic.
- **`src/encode/mod.rs`** (NEW): `pub mod pipe;`
- **`src/encode/pipe.rs`** (NEW, R4): unconditional
  `pub const H264_ENCODER: &str = "h264"`; real pipeline
  (`build_pipeline`, appsrc → videoconvert → vaapih264enc → hlsmux) gated
  behind `#[cfg(feature = "gstreamer")]`; errors via `Result<String>`, no
  unwrap/expect/panic.
- **`src/lib.rs`**: added `pub mod encode;` and `pub mod serve;`.

### Bugs found during the TDD cycle (both mine, not the spec's)
1. `CorsLayer::new()` in tower-http 0.5 defaults to NO allowed origins —
   emits only `vary`, never `Access-Control-Allow-Origin`. Fix:
   `.allow_origin(AllowOrigin::any())` (test caught it: header assert failed).
2. axum 0.7.9 routes use matchit 0.7 ⇒ params are `:name`, NOT `{name}`
   (the `{param}` syntax is axum 0.8/matchit 0.8). `/segment/{name}` 404'd
   (literal match). Fix: `/segment/:name` (test caught it: 404 vs 200).

### Outcome — gate GREEN
```
  cargo fmt --check            PASS
  cargo check                  PASS
  cargo clippy -D warnings     PASS
  cargo test                   PASS
QUALITY GATE: PASSED (rust)
```
`cargo test --test cast_tv_tests`: `test result: ok. 20 passed; 0 failed; 0
ignored; 0 measured; 0 filtered out; finished in 0.01s` — both new tests
`test_hls_playlist_has_cors ... ok`, `test_served_segment_bytes ... ok`.

### Exit criteria — all 5 pass (EC1-EC5 all exit 0)
- cargo test grep ok; both files exist; grep CORS header in server.rs; grep
  -qi h264 in pipe.rs; no unwrap/expect in src/serve + src/encode.

### Memory keys stored this session
- `chromecast-tv-mirror/implementation/part3-hls-server` (0.8) — part-3
  findings incl. the two gotchas above (CorsLayer::new() denies all origins;
  axum 0.7 = matchit 0.7 = `:param` route syntax).

### Next steps (part 4)
- `specs/01-cast-tv-terminal-part4.md`: integrate rust_cast
  `media_load` onto the device (cast feature) and feed encoder output into
  the HLS server (gstreamer feature); both stay feature-gated for CI.


---

## 2026-08-16 (third session) — spec-01 part1 implemented; gate GREEN

### What was DONE this session
- **TDD first**: appended the four part-1 contract tests to
  `tests/cast_tv_tests.rs` (T1 `test_vte_parses_ansi_into_grid`,
  T2 `test_first_frame_is_full`, T3 `test_subsequent_frames_are_diff`,
  T4 `test_rasterize_grid_to_buffer`). Confirmed red (E0432: no `emu` module)
  before any production code.
- **`src/emu/mod.rs`** (NEW): `pub mod term; pub use term::{Cell, Rgb, ScreenFrame};`
- **`src/emu/term.rs`** (NEW): `Emulator::new/with_size/parse_bytes`; `impl Perform`
  via `alacritty_terminal::vte` (vte 0.13.1, no new dep) handling `print`,
  C0 CR/LF/BS/HT, CSI CUP (`H`/`f`) + SGR (fg 30-37/90-97, bg 40-47/100-107,
  bold 1/22, reset 0); grid row-major, defaults fg 192,192,192 / bg 0,0,0
  (documented); frames diffed through the existing `damage::DamageTracker`
  (fresh tracker ⇒ first frame `full == true`); out-of-range SGR codes are
  no-ops, never panic.
- **`src/render/mod.rs`** (NEW) + **`src/render/raster.rs`** (NEW):
  `rasterize(frame, buffer)` — direct byte writes (no tiny-skia): each cell =
  8×8 tile, bg fill + `FONT8X8_BASIC` glyph stamp (MSB = leftmost column),
  fg-tinted; short buffer ⇒ no-op (never panic).
- **`src/lib.rs`**: added `pub mod emu;` and `pub mod render;`.
- **`src/render/font.rs`**: fixed ONE pre-existing typo — last glyph row
  (U+007F, index 127) ended `...0x00}` instead of `...0x00]`, so the module
  did NOT compile despite the spec's claim it was "compile-checked". No table
  data changed (still 128 entries).
- **`src/emu/term.rs` clippy fixes**: `repeat_n` (manual-repeat-n) and a
  `collapsible_match` guard on `b'\x08'`; then `cargo fmt` on tests.

### Outcome — gate GREEN
- `quality-gate.sh` → fmt PASS, check PASS, clippy PASS, test PASS,
  `QUALITY GATE: PASSED (rust)`.
- Raw test result: `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (11 damage-tracker + 4 new part-1).
- All 5 part-1 exit criteria pass (test ok; 4 files exist; `impl Perform`;
  `FONT8X8_BASIC` in raster.rs; no unwrap/expect in src/emu, src/render).

### Spec gaps found (reported; worked around minimally)
1. **`ScreenFrame` as pinned cannot express diff positions**: `cells: Vec<Cell>`
   has no coordinates, yet R7 + the rasterizer require painting only damaged
   tiles. Added `pub positions: Vec<(u16, u16)>` parallel to `cells`
   (`(col, row)`, row 0 = top). All four contract tests pass unchanged.
2. **`src/render/font.rs` did not compile** (typo above) despite the spec's
   "compile-checked" claim — one-char fix, table untouched.

### What REMAINS (next agent)
1. Part 2: capture bridge (R1) + cast sender (R6) — pinned to default features.
2. Part 3: serve (R5) + encode (R4); Part 4: full 10-test suite + no-unwrap sweep.
3. Milestone-1 device smoke test (operator): rust_cast media_load an HLS URL.

## 2026-08-16 — spec-01: parts amended for first dispatch

- **Orchestrator amended the specs** (specs are the orchestrator's — no pi work
  has touched them):
- Parent `01-cast-tv-terminal.md`: removed exit criterion 2 — it invoked the
  non-portable host path `/root/.pi/agent/skills/quality-gate/run.sh` with
  `|| true` (a rubber stamp that always passes) and duplicated the harness's own
  phase-3 gate. Made criterion 7 fail-closed: missing module dirs now FAIL the
  sweep instead of passing vacuously.
- `part1` REWRITTEN: the original had an empty TDD Contract (it would have
  dispatched nothing) and the render module (R3) was orphaned — no part assigned
  it. Part 1 is now the **terminal core**: `emu/term.rs`
  (`alacritty_terminal::vte::Perform` grid, R2) + `render/raster.rs`
  (grid→RGB via the existing `render/font::FONT8X8_BASIC`, R3), with
  full-first-then-diff via the existing `damage::DamageTracker` (R7). Four tests
  = parent tests 1–4. **No new dependency**: vte is re-exported by
  alacritty_terminal 0.24 (`pub use vte;`).
- `part2` = capture bridge (R1) + cast sender (R6); `part3` = serve (R5) + encode
  (R4); both pinned to compile under `cargo test` **default features**
  (rust_cast/gstreamer are optional deps — the real integrations sit behind
  `cfg`; the Cast `media/load` payload and an injected-discovery error path make
  the sender tests run without a device or a feature). `part4` = full 10-test
  suite + fail-closed no-unwrap sweep (R8–R11).
- **Next**: dispatch `pi-workhorse.sh run specs/01-cast-tv-terminal-part1.md`.

## 2026-08-16 (second session) — spec-02 gate GREEN; D2d fixed post-amendment

### What was DONE this session
- **Context**: tree was clean at `bb79c16` (the D2d spec amendment, spec-only).
  The code from `285a458` still had the D2d bug the amendment pins, and the
  contract's `test_duplicate_last_occurrence_change_is_damaged` did not exist.
- **`tests/cast_tv_tests.rs`**: added the D2d test FIRST (TDD red): previous
  stores `'a'` at K, call `[(K,'a'),(K,'b')]` → K reported exactly once.
  Confirmed red: `left: [] right: [CellKey { row: 0, col: 0 }]`.
- **`src/damage.rs`**: rewrote `diff` to judge damage on the *final* per-key
  value — collapse the slice into a `HashMap` (last write wins), compare each
  key against `previous`, then adopt the new map wholesale (which also forgets
  removed keys, D5). Dropped the `seen`-set/`HashSet` machinery that masked
  changed later occurrences. Removed the now-unused `HashSet` import. Still
  no unwrap/expect/panic (N2); `src/render/font.rs` untouched (N4).
- Kept the pre-existing extra `test_duplicate_key_last_write_wins` (G6).

### Outcome — gate GREEN
- `quality-gate.sh` → fmt PASS, check PASS, clippy PASS, test PASS,
  `QUALITY GATE: PASSED (rust)`.
- Raw test results (unsummed): `lib` — `0 passed`; `cast_tv_tests` — `11 passed`
  (10 contract tests + 1 extra); doc-tests — `0 passed`. All `0 failed`.
- All 9 shell exit criteria pass (test file exists; tracker/reset present;
  no panic calls; font.rs clean; Cargo.toml diff only the rustix std pin).

### What REMAINS (next agent)
1. After spec-02 lands: renderer diff consumption, then spec-01 modules
   (emu, render, capture, serve, cast, encode) per `specs/01-cast-tv-terminal.md`.
2. Milestone-1 device smoke test (operator): rust_cast media_load an HLS URL.

## Previous session (2026-08-16) — spec-02: code done, gate blocked by dependency defect (RESOLVED by amendments 6a17c38/bb79c16; retained for history)

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

---

## 2026-08-16 — Part 2 implemented (direct implementation, not pidag DAG)

### Status: GREEN

### What was done
- Implemented spec `specs/01-cast-tv-terminal-part2.md` (R1 capture bridge + R6 cast sender):
  - `src/capture/mod.rs`, `src/capture/bridge.rs` — `ByteSource` trait seam, `Bridge::poll()` drains
    available bytes into `Emulator::parse_bytes`, returns bytes fed, keeps latest `ScreenFrame`.
  - `src/cast/mod.rs`, `src/cast/sender.rs` — pure `build_media_load_request(url)` → Cast v2
    `{"type":"LOAD","media":{contentId,contentType,streamType}}`; `Sender` with injected
    `Discovery = Box<dyn FnMut() -> Result<(), CastError>>`; real rust_cast session gated behind
    `#[cfg(feature = "cast")]`; `CastError` (thiserror) with `Unreachable` variant.
  - `src/lib.rs` — added `pub mod capture; pub mod cast;`.
  - `src/emu/mod.rs` — re-exported `Emulator` at `emu::Emulator` (spec's stated path; was only at
    `emu::term::Emulator`).
- Tests (TDD-first) appended to `tests/cast_tv_tests.rs`: `test_capture_bridge_feeds_bytes_to_vte`,
  `test_cast_load_url_builds_media_load`, `test_sender_reports_unreachable`. 18/18 pass.
- All 4 spec exit criteria pass (test-ok, both files exist, wire field
  `"type": "LOAD"` in sender.rs, no unwrap/expect in src/capture|src/cast).
  Note: the orchestrator amended criterion 3 after the run — it originally
  grepped the literal `"media/load"`, which passes via doc comments only (the
  Cast v2 message type is `"type": "LOAD"`, not the string `"media/load"`).

### Quality gate
- `cargo fmt --check` PASS, `cargo check` PASS, `cargo clippy -D warnings` PASS, `cargo test` PASS.
- Raw: `test result: ok. 18 passed; 0 failed; ...` (cast_tv_tests), 0/0 lib doctests.

### Next
- Part 3: HLS HTTP server + GStreamer encode pipeline (appsrc → h264 → hlsmux) behind the
  `gstreamer` feature; then Part 4: wire rust_cast session + media_load behind `cast` feature.
- Note for pidag dev (Bug A/B from prior run): implement-iter1 still wrote no files; the direct
  implementation path above is the fallback that works.
