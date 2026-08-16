# HANDOFF — INCOMPLETE (runtime iteration-budget handoff, 2026-08-16)

**Status**: YELLOW — spec-03 (mcp-server) NOT implemented; gate fix DONE; API verification COMPLETE.
**Date**: 2026-08-16 (mid-session, tool-iteration budget reached → graceful handoff)

---

## What was DONE this session

- **GATE FIXED (2026-08-16, second session)**: quality-gate.sh was red at HEAD —
  pre-existing toolchain-upgrade breakage, NOT spec-03 code (none written yet).
  - clippy: `src/bin/castctl.rs` — `stream_type_for` + `content_type` are only used
    inside `#[cfg(feature="cast")]`; default build flagged 4 dead-code/
    unused-assignment lints. Fixed: `#[cfg(feature="cast")]` on the helper; consume
    `content_type` in the `#[cfg(not(feature="cast"))]` branch (`let _ =
    (&device, &url, &content_type);`). Behavior-preserving.
  - fmt: `cargo fmt` reflowed 5 files (castctl.rs, mirror.rs, session.rs, emu/term.rs,
    HANDOFF-incomplete.md is manual) — newer rustfmt 1.95 wraps chains/tuples the old
    committed style didn't.
  - **SPEC CONFLICT to report**: spec-03 exit criterion `! git diff --name-only |
    grep -qE '^src/(capture|emu|render|encode|serve|cast)/'` NOW TRIPS — the fmt
    reflow necessarily touches `src/cast/session.rs` + `src/emu/term.rs`. Pure
    formatting, zero semantic change (verify with `git diff -w`). The "Not modified"
    table (castctl.rs, mirror.rs) is likewise violated by necessity. N3 (gate green)
    cannot hold with those greps under the current toolchain.
  - GATE RESULT: `QUALITY GATE: PASSED (rust)` — fmt/check/clippy/test all PASS;
    `cargo test`: 34 passed (raw lines in session log).
- rmcp 3.1.2 API verified (see below, unchanged).

- Read `specs/03-mcp-server.md` in full; **no spec defects found** — every load-bearing
  claim checked against the tree (lib.rs modules, Cargo.toml deps, cast/session.rs
  `send_media_load` + `StreamType`, sender.rs `DeviceAddr::new` port 8009,
  tests/cast_tv_tests.rs:646-712 meta-test with 6 dirs).
- **rmcp 3.1.2 fetched OK** (probe at `_tmp/rmcp-probe/`, `cargo fetch` exit 0; vendored
  at `/usr/local/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-3.1.2/`).
  VERIFIED API (read from vendored source):
  - `#[tool_router(server_handler)]` on an **inherent** `impl McpServer` + `#[tool(name,
    description)]` methods → emits `impl ServerHandler`; `#[tool_handler(name =
    "cast-tv-terminal", version = ...)]` (attribute on the `impl ServerHandler` block) →
    `get_info()` uses `Implementation::new(name, version)` → **`serverInfo.name ==
    "cast-tv-terminal"`** exactly as the E2E test expects.
  - Tool fns take `Parameters<Args>` (`FromContextPart` impl) where `Args:
    DeserializeOwned + JsonSchema`; return `Result<T, E>` via `IntoCallToolResult` —
    **error branch auto-sets `is_error = Some(true)`** (tool.rs:130-144); can also return
    `CallToolResult` directly (`CallToolResult::success/error(Vec<ContentBlock>)`,
    model.rs:3882/3940; `ContentBlock::text`, model.rs:2728).
  - `ServiceExt::serve((tokio::io::stdin(), tokio::io::stdout()))` then `.waiting()`
    (service.rs:330, transport/io.rs:4, service.rs:1088). Stdio is newline-delimited JSON.
  - `Router::new(service).with_tools(IntoIterator<Item=ToolRoute<S>>)`; `ToolRouter<S>`
    is `IntoIterator` (router/tool.rs:372) and `ToolRouter + ToolRouter` (Add, :604).
- **herdr CLI verified live** (herdr at /root/.local/bin/herdr):
  - `herdr tab list --workspace w1` → JSON `{"id":"cli:tab:list","result":{"tabs":[{...,
    "tab_id":"w1:t1","label":"1 - pi","workspace_id":"w1","pane_count":1,...}],
    "type":"tab_list"}}` (result is a string; parse inner JSON).
  - `herdr tab create --workspace W --label L --no-focus` (no positional args);
    `herdr tab focus <tab_id>`; `herdr pane list --workspace W`;
    `herdr pane run <PANE_ID> <COMMAND>...` (command = remaining args, pass through).
  - Live env has `HERDR_ENV=1`, `HERDR_SOCKET_PATH=/root/.config/herdr/herdr.sock`
    (DEFAULT session — nested-herdr hazard confirmed live; Runner must env_remove ALL
    `HERDR_*` and set `HERDR_SOCKET_PATH` to the configured socket).
- Toolchain: cargo/rustc 1.95.0; `CARGO_HOME=/usr/local/cargo`. `index.crates.io`
  direct curl 404s but cargo fetch works (sparse index) — deps already cached.

## What REMAINS (next agent — start here)

1. **Write tests FIRST** per TDD Contract: `tests/mcp_tests.rs` + `[[test]]` entry in
   Cargo.toml + `pub mod mcp; pub mod mux;` in src/lib.rs. Use
   `env!("CARGO_BIN_EXE_mcp-server")` for the E2E tests. FakeRunner/FakeMux/FakeCastPort
   fakes; herdr shim script for E2E (reads `HERDR_SOCKET_PATH` env, emits canned JSON).
2. Production code (spec §Module tree): `src/mux/{mod,herdr,tmux}.rs`, `src/mcp/
   {mod,config,runner,cast,display,status,errors}.rs`, `src/bin/mcp-server.rs`.
   - mux/mod.rs: `Mux` trait (ensure_window, focus, send_text, run_command,
     close_window, list_windows, list_panes, shell_focus_line, attach_shell),
     MuxError (with raw stdout/stderr), Target/Pane/WindowInfo/PaneInfo, MuxKind,
     `open()` factory from `MUX` env.
   - mcp/mod.rs is the ONLY file importing rmcp. `#[tool_router(server_handler)]` +
   `#[tool_handler(name="cast-tv-terminal")]`. 7 tools: cast_url, cast_text,
     run_command, set_font_size, pipeline_status, restore, mirror_session.
   - runner.rs: Runner trait (run, spawn_detached) + ProcRunner — capture parent
     HERDR_* keys, env_remove them, env_set HERDR_SOCKET_PATH/DISPLAY, null stdio,
     process_group(0).
   - cast.rs: `CastPort = Box<dyn Fn(&str,&str,&str,&str)->Result<String,String>>`
     closure taking &str only (StreamType types never outside cfg(cast));
     production_cast_port() cfg-gated (Err naming "without the cast feature");
     stream_type_for: image/*→NONE, video/mp4→BUFFERED, else LIVE.
   - display.rs: set_font_size (pts 6..=32, pgrep display xterm → kill → spawn with
     `-fs`), restore (kill cycle pid + pgrep, focus first cycle window, spawn detached
     bash loop containing socket + `tab focus w1:t1`, write pid file, restart_cycle
     flag), mirror_session (`-e /bin/sh -c 'exec herdr --session <name>'` / tmux
     `attach-session -t <name> [-r]`).
   - status.rs: JSON text block (mux session, windows/panes, live processes incl.
     xterm `-fs`, HLS dir state) — every field degrades to null/absent.
3. **E2E acceptance order (spec R10)**: ship stub first (cast_text does
   `println!` + `.unwrap()`) → observe `test_e2e_tool_error_keeps_server_alive` fail →
   fix → paste both outputs in final report.
4. EDIT `tests/cast_tv_tests.rs` meta-test: module_dirs += `"src/mcp","src/mux"`,
   `checked_dirs.len()` 6→8 (lines ~648-655, ~707). Only change allowed.
5. Gate: `bash .orchestration/lib/quality-gate.sh /projects/chromecast-tv-mirror`
   (stages: cargo fmt --check / check / clippy -D warnings / test -j 2) + the spec's
   exit-criteria greps (`! grep -rn 'println!(\|print!(' src/mcp src/mux
   src/bin/mcp-server.rs` etc). Report raw `test result:` lines, unsummed.
6. Update HANDOFF.md via handoff-generator; store memory keys below.

## Key files
- `specs/03-mcp-server.md` — spec + TDD contract + exit criteria (read first)
- `_tmp/rmcp-probe/` — probe crate proving rmcp 3.1.2 fetch (deps now in cargo cache)
- `src/cast/session.rs` — `send_media_load(device, url, ct, StreamType)` signature to reuse
- `tests/cast_tv_tests.rs:646` — meta-test to extend (6→8 dirs)

## Memory keys (session) — store BEFORE declaring done
- `chromecast-tv-mirror/implementation/rmcp-3.1.2-api` (0.8): tool_router(server_handler)
  + tool_handler(name=...) → serverInfo.name; Parameters<>; Result<T,E> error branch
  auto-sets is_error; serve((stdin,stdout)).waiting(); Router::with_tools(IntoIterator).
- `chromecast-tv-mirror/implementation/herdr-cli-shapes` (0.7): tab list/create/focus,
  pane list/run argv shapes + JSON envelope `{"id","result","type"}`; HERDR_SOCKET_PATH
  honored via env; live default-session socket path (nested-herdr hazard).
