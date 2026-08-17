# HANDOFF — chromecast-tv-mirror

**Status**: GREEN — **spec-03 part 3 (E2E stdio + R10 acceptance) LANDED**
(2026-08-17): both E2E tests spawn the real `mcp-server` binary against the
fake herdr shim; wrong-version-first proof done (stub println+unwrap failed
both, fixed version passes); 52/52 tests; gate exit 0; all 13 exit criteria
green. Prior baseline: spec-03 part 2 landed (McpServer + 7 tools +
`serve_stdio()`), gate 13/13.

---

## 2026-08-17 — SPEC-03 PART 3 LANDED: E2E over a real stdio pipe (R1/R10/N4)

### What was DONE this session
- **TDD first**: appended the two part-3 contract tests to `tests/mcp_tests.rs`
  (only tracked change; `git diff` touches nothing else — no `src/` file,
  `Cargo.toml`, or spec).
  - `test_e2e_stdio_handshake` (R1): spawns `env!("CARGO_BIN_EXE_mcp-server")`
    with `MUX=herdr`, `MUX_SOCKET` → fake path, `HLS_DIR` → scratch, `PATH`
    first entry = temp dir holding a `herdr` symlink → the shim, `FAKE_LOG` →
    scratch; drives newline JSON-RPC `initialize` (asserts
    `serverInfo.name == "cast-tv-terminal"` + `capabilities.tools` present) →
    `notifications/initialized` → `tools/list` (all 7 names) → `tools/call
    cast_text` (success text names the window AND `FAKE_LOG` contains the
    `pane run` + `tab focus w1:t1` invocations) → process alive.
  - `test_e2e_tool_error_keeps_server_alive` (R10 ACCEPTANCE): same spawn with
    `MUX_SOCKET` = a path carrying the `fail` marker → shim exits non-zero on
    every command → cast_text answers a well-formed `isError:true` tool result
    (NOT a JSON-RPC error, NOT a hang), then `tools/list` is answered again
    and `try_wait()` proves the process is alive.
  - E2E harness: `E2eFixture` (unique scratch under temp_dir: bin symlink +
    HLS playlist + fake log) + `E2eServer` (tokio child, piped stdio, stderr
    drained to a file, 20 s read timeouts, kill_on_drop). Every stdout line is
    parsed as JSON — any non-protocol byte fails the test (N4).
- **`tests/fixtures/fake-herdr.sh`** (NEW, executable): logs every invocation
  to `$FAKE_LOG`, answers the driver's contract (`tab list` → existing `agent`
  tab `w1:t1`; `pane list` → `w1:p1`; create/focus/close/run → empty result);
  any `HERDR_SOCKET_PATH` containing `fail` → exits 1 with a missing-socket
  error on stderr. Never touches the live stack (G7).
- **Wrong-version-first proof (spec prose criterion 1)** — stub `cast_text`
  with `println!("handling cast_text")` + `.unwrap()` on the mux call:
  - `test_e2e_stdio_handshake` → FAILED:
    `cast_text must be answered: "stdout carried a non-JSON-RPC line \"handling cast_text\\n\" (expected value at line 1 column 1)"`
  - `test_e2e_tool_error_keeps_server_alive` → FAILED:
    `the failing cast_text must still be answered, not hang: "stdout carried a non-JSON-RPC line \"handling cast_text\\n\" ..."`
  - Reverted the stub (no `src/` diff remains); fixed version → both PASS.

### Outcome — gate GREEN (exit 0)
```
  cargo fmt --check            PASS
  cargo check                  PASS
  cargo clippy -D warnings     PASS
  cargo test                   PASS
QUALITY GATE: PASSED (rust)
```
Raw (unsummed): `cargo test` → lib 0/0, castctl 0/0, mcp-server 0/0, mirror
0/0, `tests/cast_tv_tests.rs`: `test result: ok. 34 passed; 0 failed; 0
ignored; 0 measured; 0 filtered out`, `tests/mcp_tests.rs`: `test result: ok.
18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. Non-vacuous
greps: `test_e2e_stdio_handshake` → `ok. 1 passed`, `test_e2e_tool_error_keeps_server_alive`
→ `ok. 1 passed`, `test_no_production_unwrap` → `ok. 1 passed` (8 dirs).

### Exit criteria — all 14 pass
`cargo build` / `--features cast` / `--features cast,gstreamer` compile;
`cargo test` green; the three non-vacuous test greps exit 0; shim exists +
executable; clippy/fmt clean; no `println!`/`print!` and no
`/projects/...|/root/` in `src/mcp src/mux src/bin/mcp-server.rs`;
`git diff --name-only` = `tests/mcp_tests.rs` only (fixture untracked) — no
prior module or spec touched.

### Spec notes (no defects)
- rmcp 3.1.2 stdio is newline-delimited JSON-RPC (verified in the crate's
  `transport/async_rw.rs` Lines codec) — the E2E's line-based framing matches.
- `#[tool_router]` always injects `capabilities.tools` (list_changed) into the
  initialize result — the `capabilities.tools` assert is stable.

### What REMAINS (next)
1. Orchestrator: commit part 3, move master spec-03 status to IMPLEMENTED.
2. **Phase 8 — live TV verification** (orchestrator/operator step): `claude
   mcp add` against the built binary, then cast_url/cast_text/set_font_size/
   mirror_session/restore on the TV + tmux parity. NOT part of any spec
   dispatch.
3. (Log) EXPERIMENT LOG: append this session's observation — rmcp's line
   parser rejects any non-JSON stdout byte instantly (E2E caught the stub
   println), confirming N4's stdio discipline is enforceable by test.



## 2026-08-17 — SPEC-03 PART 2 LANDED: McpServer struct + 7 tools + stdio entrypoint

### What was DONE this session
- **TDD first**: appended 7 part-2 contract tests to `tests/mcp_tests.rs`
  (`test_tool_router_registers_all_tools`, `test_cast_url_forwards_to_port`,
  `test_set_font_size_relaunch`, `test_set_font_size_rejects_range`,
  `test_restore_focus_and_cycle`, `test_mirror_session_relaunch`,
  `test_pipeline_status_json`) using `FakeRunner`/`FakeMux`/`FakeCastPort`
  (first run: 15/16 red — status test caught a pgrep-fetch-ordering bug).
- **`src/mcp/mod.rs`** (REPLACED, keeps part-1 decls + adds `display`/`status`):
  `McpServer { Arc<Config>, Arc<dyn Runner>, Arc<dyn Mux>, CastPort }`,
  `#[tool_router(vis = "pub")]` over an inherent impl with all 7 `#[tool]`
  methods, `#[tool_handler(name = "cast-tv-terminal", ...)] impl ServerHandler`,
  `serve_stdio()` (`serve(stdio()).await?.waiting().await?`). The ONLY file
  importing rmcp. `use rmcp::schemars;` needed so the JsonSchema derive finds
  `schemars::` paths; tool fn `version` attr can't take `env!()` (macro-quote
  limitation) — hardcoded `"0.1.0"` matching Cargo.toml.
- **`src/mcp/display.rs`** (NEW, R6/R8/R9): `set_font_size` (pts 6..=32
  validation → kill xterm via `pgrep -f herdr-tv` → relaunch with `-fs`,
  `DISPLAY=:99`, geometry, `HERDR_*` removed), `restore` (kill pid-file pid +
  `pgrep` fallback → optional respawn of detached `bash -c while … tab focus
  w1:tN … sleep 10` loop with the socket baked in → write pid → `mux.focus`
  first cycle window; `restart_cycle=false` → no spawn), `mirror_session`
  (kill xterm → optional `mux.focus(window)` → spawn xterm with the driver's
  `attach_shell`: `exec herdr --session '<name>'` / `exec tmux attach -t
  '<name>' -r`). All spawns via the `Runner` (null stdio, own process group,
  env stripped) — never inline `Command`.
- **`src/mcp/status.rs`** (NEW, R7): `pipeline_status_json()` — mux session +
  windows/panes, processes (Xvfb, display xterm incl. `-fs` parsed from its
  `pgrep -af` cmdline, ffmpeg, hls_server, cycle loop), HLS dir (playlist
  presence + 5-line tail, segment count, newest segment by mtime). Every
  piece degrades to null; no unwrap/panic. Gotcha: pgrep fetch order must
  match JSON field order (json! evaluates bindings first) — test caught it.
- **`src/bin/mcp-server.rs`** (NEW, R1): stderr-only `log` logger,
  `Config::from_env()`, `mux::open()`, `production_cast_port()`, wiring,
  `serve_stdio()`. `log::set_logger` error mapped (SetLoggerError isn't
  `std::error::Error`).
- **Cargo.toml / src/lib.rs / src/mux / part-1 files: untouched** (N2).

### Outcome — gate GREEN (exit 0)
```
  cargo fmt --check            PASS
  cargo check                  PASS
  cargo clippy -D warnings     PASS
  cargo test                   PASS
QUALITY GATE: PASSED (rust)
```
Raw (unsummed): `cargo test` → lib 0/0, castctl 0/0, mcp-server 0/0, mirror
0/0, `tests/cast_tv_tests.rs`: `test result: ok. 34 passed; 0 failed; 0
ignored; 0 measured; 0 filtered out`, `tests/mcp_tests.rs`: `test result: ok.
16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
Non-vacuous greps: `test_tool_router_registers_all_tools` → `ok. 1 passed`,
`test_no_production_unwrap` → `ok. 1 passed` (8 dirs, no unwrap in the new
files).

### Exit criteria — all pass (after orchestrator fixes)
The workhorse handed off at **12/13**: criterion 9 `cargo clippy --all-targets
-- -D warnings` failed on a `type_complexity` lint in `test_cast_url_forwards_to_port`
(`Arc<Mutex<Vec<(String, String, String, StreamKind)>>>` — its own gate ran
`cargo clippy` without `--all-targets`, which skips the integration tests). The
orchestrator fixed it with a `CastRecord` type alias, and also fixed the
review-flagged latent tmux bug in `cycle_loop_argv` (it built herdr `w1:tN`
window ids for the tmux driver too → `tmux select-window -t tv-demo:w1:t1`;
now 0-based indices for tmux). Re-run: all 13 green.
The rest as handed off: `cargo build`, `--features cast`, `--features
cast,gstreamer` all compile; the three non-vacuous test greps exit 0;
`src/bin/mcp-server.rs` present + `serve` in mod.rs; no
`println!`/`print!` in src/mcp|src/mux|bin; no
`/projects/chromecast-tv-mirror|/root/` in those dirs; `git diff --name-only`
touches only `src/mcp/{mod,display,status}.rs`, `src/bin/mcp-server.rs`,
`tests/mcp_tests.rs`. lib.rs module list unchanged (exactly capture, cast,
damage, emu, encode, pipeline, render, serve, mcp, mux).

### Spec notes (no defects)
- rmcp 3.1.2 `#[tool_handler]` `version` attr does not accept `env!()` (macro
  can't parse it) — used the literal `"0.1.0"` (crate version).
- `#[tool]` fns must be `pub` for the integration tests to call them
  (external-crate visibility), matching the TDD contract's direct-call rows.
- `Runner` signature `remove_env: &[&str]` requires owned `herdr_env_keys()`
  to be bound to a local before slicing (E0716) — cosmetic, not spec-relevant.

### What REMAINS (next agent)
1. **Part 3** (`specs/03-mcp-server-part3.md`): E2E stdio tests
   (`test_e2e_stdio_handshake`, `test_e2e_tool_error_keeps_server_alive`)
   driving the real built binary — the R1/R10 acceptance proof.
2. Then phase-8 live TV verification (orchestrator) + tmux parity check.


---

## 2026-08-16 — SPEC-03 (mcp-server) PART 1 LANDED: mux dual-driver + seams

> Current work is **spec-03 (MCP-over-stdio Chromecast control server)**,
> split into 3 parts (commit `889848b`; master spec
> `specs/03-mcp-server.md` is the source of truth).
>
> **Part 1 is DONE** — implemented by the ORCHESTRATOR (operator: "do it
> yourself this time only!"), committed `d539eba`, 13/13 exit criteria green
> (build / --features cast / --features cast,gstreamer / test 34+9 / clippy
> -D warnings / fmt). What landed: `src/mux` (`Mux` trait + `HerdrMux`/
> `TmuxMux`, `shell_single_quote`, lazy `open()`), `src/mcp` (`McpServerError`,
> `Config::from_env`, `Runner`/`ProcRunner`, `CastPort`+`stream_type_for`),
> `rmcp 3` in Cargo.toml (resolves against the rustix `std` pin), 9 part-1
> unit tests (acceptance `test_runner_removes_herdr_env` uses the REAL
> `ProcRunner`), meta-test walks 8 dirs.
>
> **Verified live during implementation** (read-only): herdr JSON contract is
> `result.tabs[].tab_id|label` and `result.panes[].pane_id|tab_id` — NOT the
> guessed `id`/`window_id` top-level shape; both drivers parse the verified
> shapes. herdr CLI confirmed: `tab create --workspace/--label/--no-focus`,
> `tab focus <id>`, `tab close <id>`, `pane run <id> <cmd>...`.
>
> **Next: dispatch part 2** (`specs/03-mcp-server-part2.md`) via
> `pi-workhorse.sh` — the `McpServer` struct, 7 tools, `ServerHandler`,
> `serve_stdio`, `bin/mcp-server.rs`. Part 3 (E2E stdio) after that. Phase 8
> (live TV verify + tmux parity) runs when all three parts land.
>
> Prior stalled-run research: `HANDOFF-incomplete.md` (rmcp 3.1.2 API,
> herdr CLI argv/JSON, implementation order).

---

## 2026-08-16 (milestone-2 operator test session) — LIVE TEXT ON TV; two rendering bugs fixed

### Cast ladder — every rung proven on the device (10.10.10.208, 720p)
1. **image/jpeg** (`castctl --image`) → solid pink screen (cast leg works).
2. **video/mp4 + BUFFERED** (`--type video/mp4`) → rainbow testsrc2 plays.
3. **VOD HLS** (`application/vnd.apple.mpegurl` + LIVE) → 10-segment film plays.
4. **LIVE HLS** from the running `mirror` → text reaches the TV.

### Root causes discovered on-device (all committed with explanatory comments)
- **Content type**: the DMR rejects `application/x-mpegURL` in a custom-sender
  media/load — picks no HLS player, never fetches the manifest. Canonical
  `application/vnd.apple.mpegurl` + `StreamType::Live` plays (963553a).
- **Silent AAC is MANDATORY**: the DMR refuses video-only HLS (video-only
  fetched 0 segments VOD / stalled after 2 live; same film with audio played
  all). `hlssink2` video+audio pads, `audiotestsrc wave=silence` → `voaacenc`
  (f924c57).
- **`\n` must reset the column** (`a6ab372`): the emulator treated LF as a bare
  line feed (row+1 only), so every line of a bare-LF `--source` started where
  the previous line ended → diagonal "staircase", wrapping at the right edge.
  A tty applies ONLCR so real pipe-pane output (CRLF) worked; the plain-text
  test file exposed it. Verified via emulator grid dump + encoded-frame cell map.
- **Font is LSB-first** (`995f7ef`): Hepper's `font8x8_basic` stores each glyph
  row with **bit 0 = leftmost**, but `paint_tile` read MSB-first (`0x80 >> gx`),
  flipping every glyph horizontally ("mirroed!"). Now `1 << gx`. The rasterize
  test asserted the old (wrong) MSB interpretation — corrected.

### Runtime topology (container)
- `mirror` runs in-container on 0.0.0.0:18080; the operator device fetches
  `http://10.10.10.217:18080/live.m3u8` via the host `socat` bridge
  (18080→18081) + the container's reverse `ssh -N -R` tunnel. Verified frames:
  encoded segment cell-map matches the emulator grid byte-for-byte (rows,
  columns, and per-glyph pixels).
- Operator workflow: build `--features cast,gstreamer`; restart `mirror` with
  the same args to pick up a rebuilt binary (kills the old test process only —
  never the host socat/tunnel, herdr, or llama-server).

### Next steps
- Feed a **real herdr/tmux pipe-pane** as `--source` for dynamic content.
- Audio (user: "we will check the audio later").
- Then pidag (53 specs).

---

## 2026-08-16 (part 6 session) — full pipeline integration; IMPLEMENTED + phase-7 committed

### What was DONE this session
- **TDD first**: appended 8 part-6 contract tests to `tests/cast_tv_tests.rs`
  (suite now 29 = 27 sync + 2 async — a plain `grep "^fn test_"` counts 27 and
  misses the async ones). `test_pipe_source_reads_available_bytes` (7 bytes,
  EOF→0), `test_null_encoder_counts_frames` (3 submits, stream URL),
  `test_pipeline_submits_changed_frames` / `_skips_unchanged_frames` /
  `_keepalive_after_idle` (cadence: changed→submit, unchanged+pre-keepalive→
  skip, idle past 1000 ms→exactly one keepalive frame), `test_dir_store_
  reads_output_dir`, plus async `test_served_playlist_reads_from_store`
  (AMENDS part-3's CORS/segment tests → store-driven, asserts 200 + CORS + 404)
  and `test_serve_hls_binds_and_responds` (real `serve_hls` entry, HTTP 200).
  Raw `TcpStream` GET (`raw_get`) — no dev-deps.
- **`src/capture/pipe.rs`** (NEW, R1): `PipeSource` — reads a tmux/herdr
  `pipe-pane` output file; a regular-file read never blocks and returns 0 at
  EOF, so no `O_NONBLOCK`/libc dep is needed.
- **`src/pipeline/`** (NEW): `PipelineConfig {keepalive_ms: 1000, tick_ms: 10}`,
  `Pipeline<S: ByteSource, E: Encode>::poll_and_submit(now_ms)` — poll → diff →
  rasterize (grid×8 RGBA) → `submit_frame`; skips unchanged frames before the
  keepalive deadline, pushes one keepalive frame after. `run()` loops with
  `tokio::signal::ctrl_c` shutdown; errors logged, never exit.
- **`src/encode/pipe.rs`** (MODIFY, R4 rework): `Encode` trait;
  `NullEncoder` (in-container, does NOT validate buffer size — the TDD test
  feeds a 4-byte buffer for a claimed 8×8 canvas); `#[cfg(feature="gstreamer")]`
  `GstEncoder` + `build_pipeline`: appsrc → videoconvert → x264enc/vaapih264enc
  → **hlssink2** (NOT `hlsmux` — not an element name) → `seg_%05d.ts` +
  `live.m3u8`, `playlist-root` for absolute segment URLs. Buffer-size validated,
  PTS from running timestamp, `Drop` → State::Null.
- **`src/serve/store.rs`** (NEW, R5): `MediaStore {playlist, segment}`;
  `MapStore` (in-memory, seeded — tests + dry-run) and `DirStore` (production,
  reads hlssink2 output dir, traversal guard rejects `/ \ ..`).
- **`src/serve/server.rs`** (MODIFY): store-driven `app(Arc<dyn MediaStore>)` —
  `/live.m3u8` + `/segment/:name`, 404 on None, CORS any-origin; `serve_hls`
  runs `axum::serve`.
- **`src/bin/mirror.rs`** (NEW): operator binary — `--source` (required),
  `--bind A:P`, `--size WxH` (160×45), `--outdir`, `--encoder x264|vaapi`,
  `--device IP`, `--url-base`, `--no-cast`; `--help` → usage, exit 0; usage
  errors → exit 2; wildcard-bind + device without `--url-base` → exit 2.
  Default features: `MapStore::seeded` placeholder + `NullEncoder` = dry-run
  (`curl http://127.0.0.1:8080/live.m3u8` → 200). gstreamer feature: `DirStore`
  + `GstEncoder`, creates outdir/segment, ROOT = url-base minus `/live.m3u8`
  plus `/segment`. `cast_to` once, non-fatal (R11).

### ORCHESTRATOR phase-7 fix — real runtime bug the gate/review missed
- `mirror` **panicked at runtime** on `tokio::net::TcpListener::from_std` of a
  listener bound OUTSIDE the runtime ("Registering a blocking socket with the
  tokio runtime is unsupported"). Second latent bug under it: after
  `rt.spawn(serve_hls)`, the sync `pipeline.run()` blocked the main thread, so
  the runtime was never polled again — the server would never have served.
  Fix: bind with `tokio::net::TcpListener::bind` INSIDE the runtime
  (`rt.block_on`), and drive `pipeline.run()` on its own std thread while the
  main thread `rt.block_on`s `ctrl_c`. Verified end-to-end: `curl` → 200 + CORS,
  SIGINT → graceful exit 0. Unit tests did not catch this because they drive
  the server inside a test runtime; the binary's runtime topology is only
  exercised by running it.
- **Spec amendment**: TDD row `test_pipe_source_reads_available_bytes`
  said "returns 5" for `b"hi\x1b[31m"` (7 bytes) — corrected to 7 to keep spec
  ↔ code truthful.

### Outcome — independent orchestrator re-verification
- `cargo test` default: **29/29 pass**; `cargo test --features cast`:
  **28/28** (device-address test gated off — its contract is default-only).
- `mirror --help` exit 0; dry-run `mirror --source <pane> --bind 127.0.0.1:18080`
  → GET /live.m3u8 200 (playlist), /segment/seg0.ts 200, `access-control-allow-
  origin: *`, SIGINT exit 0.
- Workhorse gate (fmt/check/clippy/test) GREEN, review PASS (static), validate
  8/8, all exit criteria green. Committed **`a40e4f8`** (orchestrator phase 7:
  implementation + fix + spec amendment).

### Next steps
- **Milestone-2 operator test on a LAN-reachable host**: build with
  `--features cast,gstreamer`, then
  `mirror --source <herdr/tmux pipe-pane file> --bind 0.0.0.0:8080 --outdir
  <dir> --url-base http://<LAN-IP>:8080/live.m3u8 --device 10.10.10.208`
  — live pane should appear on the TV.
- Then pidag (53 specs).


---

---

## 2026-08-16 (milestone-1 smoke test session) — PASS; one protocol bug fixed

### What was DONE this session
- **Built** `castctl` with `--features cast` (rust_cast 0.17 + openssl), ran
  the milestone-1 smoke test against the operator device `10.10.10.208`
  (Chromecast, MAC `54:60:09:DE:4D:24`).
- **First run — HANG (diagnosed)**: used the canonical Big Buck Bunny URL
  `http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.m3u8`.
  The DMR launched (TV woke, Cast logo) but `media.load` blocked forever
  (timeout 124) and nothing played. Root causes found:
  1. **That URL is DEAD** — `commondatastorage.googleapis.com` returns
     `403 AccessDenied` (anonymous access locked down) as of 2026. Confirmed
     via curl from the container.
  2. **A real protocol bug** (the hang): rust_cast's `media.load` waits
     indefinitely in `receive_find_map` for a STATUS whose request_id matches
     or whose media entry content_id matches. If the DMR drops the LOAD
     entirely, NO status ever arrives → infinite hang. The DMR drops LOADs on
     an unconnected app transport. **Canonical flow requires
     `connection.connect(&application.transport_id)` AFTER `launch_app` and
     BEFORE `media.load`** (verified against upstream rust_caster example:
     `launch_app -> connection.connect(transport_id) -> media.load`). Our
     session skipped that step.
- **Fix** (`55c9a05`, committed): added the app-transport `connect` to
  `src/cast/session.rs` step 2b, with the upstream-verified ordering noted in
  the comment. Also gated `test_sender_accepts_device_address` to
  `#[cfg(not(feature = "cast"))]` — under the cast feature its injected fake
  address triggered a REAL connect to `192.168.1.50:8009` (≈130 s hang). The
  test's spec contract is default-features-only ("session compiled out, no
  network"); the live path is covered by the device smoke test.
- **Second run — PASS**: `castctl 10.10.10.208
  "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8"` →
  `castctl: PASS — media load sent` (exit 0, `load` returned `Ok(Status)`).
  **Big Buck Bunny played on the TV** (operator-confirmed).

### Working HLS URLs (2026-08-16)
- ✅ `https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8` — HTTP 200, no
  redirect, `audio/mpegurl`, 240p–1080p variants.
- ❌ `commondatastorage.googleapis.com/.../BigBuckBunny.m3u8` — 403 now.
- ⚠️ Apple `devstreaming-cdn.apple.com/.../img_bipbop...m3u8` — 302 redirect.
- ❌ `bitdash-a.akamaihd.net/content/sintel/hls/playlist.m3u8` — 403 now.

### Milestone status
- **PIVOT GUARDRAIL: not triggered.** rust_cast CAN `media_load` HLS onto a
  real device. Option-2 (custom receiver / registration / WebRTC) stays out of
  scope for milestone 1.
- Tests: 23/23 default, 22/22 `--features cast` (gated test excluded).
- Agent-memory (topic `chromecast-tv-mirror`) stored:
  `implementation/rustcast-transport-connect` (0.9),
  `implementation/milestone1-smoke-pass` (0.9).

---


## 2026-08-16 (part 5 session) — real rust_cast session + castctl; gate GREEN

### What was DONE this session
- **TDD first**: appended the two part-5 contract tests to
  `tests/cast_tv_tests.rs` — `test_sender_accepts_device_address` (discovery
  yields `Ok(DeviceAddr)` ⇒ `send_load` returns `Ok(())` under default
  features, session compiled out, no network) and `test_device_addr_default_port`
  (`DeviceAddr::new("10.0.0.5")` ⇒ port 8009, host kept). Existing
  `test_sender_reports_unreachable` passes UNCHANGED (the always-Err closure
  infers the amended result type). Also updated the `cast` import to include
  `DeviceAddr`. 23/23 pass.
- **`src/cast/sender.rs`** (MODIFY): added `DeviceAddr {host, port}` +
  `DeviceAddr::new` (port 8009); amended `Discovery` to
  `Box<dyn FnMut() -> Result<DeviceAddr, CastError>>`; `send_load` now takes
  the discovered device, calls the session behind `#[cfg(feature = "cast")]`,
  and silences the unused device under `#[cfg(not(feature = "cast"))]` with
  `let _ = &device;` (clippy -D warnings stays green).
- **`src/cast/session.rs`** (NEW, `#[cfg(feature = "cast")]`): real rust_cast
  0.17.0 session verified against crate source — `CastDevice::
  connect_without_host_verification` (self-signed certs expected) →
  `connection.connect("receiver-0")` → `heartbeat.ping()` →
  `CastDeviceApp::from_str("CC1AD845")` (rust_cast has NO `From<&str>` for
  `CastDeviceApp`, only `FromStr` — the spec's `&"CC1AD845".into()` sketch
  cannot compile as written) → `receiver.launch_app` → `media.load(
  &application.transport_id, &application.session_id, &Media{content_id: url,
  stream_type: Live, content_type: "application/x-mpegURL"})` — destination is
  the launched app's transport protocol (e.g. `web-1`); `load`'s two args must
  be the SAME string type (`&String`/`&str` unify). Every `rust_cast::Error`
  mapped via `map_err` to `CastError::Session`; zero unwrap/expect.
- **`src/cast/mod.rs`** (MODIFY): `pub use sender::{CastError, DeviceAddr,
  Sender};` + `#[cfg(feature = "cast")] pub mod session;`.
- **`src/bin/castctl.rs`** (NEW): `castctl <device-ip> <hls-url>` — builds
  `Sender::new(Box::new(move || Ok(DeviceAddr::new(ip.clone()))))` (FnMut ⇒
  clone inside the closure; the spec's literal `move || Ok(DeviceAddr::new(ip))`
  does not compile — E0507), prints device+url under `cast` / the
  "built without the cast feature — no session will be sent; rebuild with
  --features cast" notice under default features, and exits non-zero on
  failure (ExitCode, no unwrap/expect/panic). Manual check: `./target/debug/
  castctl 10.10.10.208 http://tv:8080/live.m3u8` prints the notice, exit 1.
- **No Cargo.toml changes** (rust_cast already an optional dep); `cargo check
  --features cast` FINISHED cleanly (pkg-config/openssl present).

### Outcome — gate GREEN (exit 0)
```
  cargo fmt --check            PASS
  cargo check                  PASS
  cargo clippy -D warnings     PASS
  cargo test                   PASS
QUALITY GATE: PASSED (rust)
```
Raw: `test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0
filtered out; finished in 0.01s` (cast_tv_tests); 0/0 lib + 0/0 castctl bin.

### Exit criteria — all 5 pass
- EC1 cargo test grep ok; EC2 `cargo check --features cast` → Finished;
- EC3 session.rs/sender.rs/castctl.rs exist; EC4 `CC1AD845` in session.rs;
- EC5 no `.unwrap()`/`.expect()` in src/cast src/bin (grep exit 1 as
  required).

### Spec deviations (code-level, spec's own notes anticipated them)
- `&"CC1AD845".into()` → `CastDeviceApp::from_str("CC1AD845")` (no From<&str>
  impl in rust_cast 0.17.0).
- `move || Ok(DeviceAddr::new(ip))` → `move || Ok(DeviceAddr::new(ip.clone()))`
  (FnMut closure cannot move out of its capture).
- `media.load(destination, ...)`: destination = `&application.transport_id`
  (the launched app's protocol id), both args `&String`.

### Memory keys stored this session
- `chromecast-tv-mirror/implementation/part5-real-session` (0.8) — rust_cast
  0.17.0 session flow verified from source; the two spec-sketch compile fixes
  (FromStr vs Into; FnMut clone); media.load destination = transport_id.

### Next steps
- Operator: `cargo build --release --features cast` then
  `castctl 10.10.10.208 http://<host>:8080/live.m3u8` — milestone-1 device
  smoke test. If rust_cast cannot media_load HLS onto the device, STOP and
  report for explicit Option-2 decision (pivot guardrail).
- Later: feed real encoder output into the HLS server (gstreamer feature).


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
- `chromecast-tv-mirror/implementation/part2-capture-cast` (0.8) —
  orchestrator-stored: the capture/cast implementation + the media/load
  wire-field correction.
- `chromecast-tv-mirror/implementation/spec01-complete` (0.85) —
  orchestrator-stored: spec-01 completion status (six modules, 21 tests,
  default-feature compile, Cargo.toml untouched) + the review-caught-spec-
  defect-every-part orchestration pattern.

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

---

## 2026-08-16 (framebuffer milestone) — REAL htop pane on the TV; escape-parsing abandoned

### What changed
The live text pane moved from the emulator re-parsing a byte stream to **real
framebuffer capture**: Xvfb + xterm running actual `htop`, ffmpeg `x11grab`
encodes the X screen to HLS, the DMR casts it. Whatever is on the X screen is
cast — there is no parser to drift. Operator-confirmed: "everything is there!",
then font quality fixed: "fonts are better... almost usable".

### Why the pivot
Real ncurses apps (htop) only rewrite *changed* cells with the full CSI set.
The hand-rolled emulator missed CHA/VPA/EL/ED (added in `d61e4b4`, 33/33
tests) but the class of bug — divergence never corrected, because ncurses never
re-emits untouched cells — is inherent to parsing. The framebuffer has no such
class.

### Live framebuffer stack (container; restart order matters)
- `Xvfb :99 -screen 0 1280x720x24 -ac -nolisten tcp`
- `DISPLAY=:99 xterm -class XTerm -fn 8x13 -geometry 159x55+0+0 -xrm
  'XTerm*background: black' -xrm 'XTerm*foreground: white' -e htop`
  — 8×13 misc-fixed fills 1280×720 at 159×55. 6×13 was too thin (1px strokes
  = blur under x264); 8×13 gives 2px strokes. xterm defaults to a WHITE
  background — must pass black resources.
- ffmpeg x11grab → HLS, **silent AAC mandatory** (video-only HLS is refused by
  the DMR, re-confirmed):
  `-f x11grab -video_size 1280x720 -framerate 10 -draw_mouse 0 -i :99 -f lavfi
  -i anullsrc=channel_layout=stereo:sample_rate=44100 -map 0:v -map 1:a
  -c:v libx264 -preset medium -tune zerolatency -pix_fmt yuv420p -crf 16
  -g 10 -sc_threshold 0 -deblock 0 -c:a aac -b:a 128k -f hls -hls_time 1
  -hls_list_size 6 -hls_flags delete_segments -hls_base_url
  http://10.10.10.217:18080/ /tmp/m2/xhls/live.m3u8`
- python `hls_server.py` serves /tmp/m2/xhls on 0.0.0.0:18080 with
  `application/vnd.apple.mpegurl` (.m3u8) / `video/mp2t` (.ts) + CORS.
- Same reverse tunnel as before (host socat 18080→18081 + container
  `ssh -N -R`) delivers it to 10.10.10.208.

### Font-blur fix (measured)
Edge-sharpness (per-pixel vertical-gradient energy), same frame through each
setting: old `-preset veryfast -b:v 2M` = **79%** of raw; `-preset medium
-crf 18` = 98%; `-crf 16 -deblock 0` (live now) = **98.3%**. The 2M veryfast
encode was the blur; deblock off keeps thin glyph edges crisp.

### Next steps
- Audio check (user: "we will check the audio later").
- Then pidag (53 specs).
- Rust-native mirror alternative recommended (not started): `vt100` crate +
  `fontdue` TTF rasterizer → 1280×720 RGBA in-container, no Xvfb/apt. Note:
  Xvfb/xterm/fonts were apt-installed — overlayfs `/` means they do NOT
  survive a container reboot; only `/root` and `/projects` persist.

---

## 2026-08-16 (TrueType fonts on the framebuffer) — DejaVu Sans Mono replaces bitmaps; operator-confirmed "perfect"

### What changed
The live xterm moved from X11 bitmap fonts (6×13 / 8×13) to **TrueType via
xterm's freetype+fontconfig** (XTerm 398). Side-by-side TV A/B: 6×13 bitmap
(left) vs DejaVu Sans Mono 13pt (right) → operator: "fonts on the right are
much sharper", then full-screen confirmed "perfect".

- `DISPLAY=:99 xterm -class XTerm -fa 'DejaVu Sans Mono' -fs 13
  -geometry 116x32+0+0 ... -xrm 'XTerm*background: black' -xrm
  'XTerm*foreground: white' -e htop`
- **xterm `-fs` is in POINTS, not pixels** — 13pt ≈ 11×22 px cell (so 116×32
  fills 1280×720). `-fs 15` → 13×25 cell, oversized/clipped. For a denser
  ~160×55 you'd need ~7-8pt (strokes thin back toward blur); 16pt → ~94×26
  chunkier.
- Installed mono fonts: DejaVu Sans Mono (TTF), Noto Sans Mono (TTF), Nimbus
  Mono PS (URW base35 — the "Adobe" PostScript Courier clone, OTF/Type1).
- Why sharper: anti-aliased ≥2px strokes survive x264 CRF16 + `-deblock 0`,
  where 1px bitmap strokes soften.

### Next steps
- Audio check, then pidag (53 specs).
- Future research (see agent-memory `research/*`): HTML+JS graphical dashboard
  on the TV; agent-managed pipeline (verdict: tmux via Claude Code Bash — no
  MCP/ACP/A2A needed; see `research/agent-managed-pipeline-protocols`).
