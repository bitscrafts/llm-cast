# HANDOFF — chromecast-tv-mirror

**Status**: YELLOW — pidag implementation relaunched post-pi-upgrade; worker DAG is
RUNNING in background, outcome not yet confirmed at time of handoff.
**Date**: 2026-08-10 (pi upgraded to 0.2.0, compaction bug fixed)
**Phase**: 3 — TDD implementation of `specs/01-cast-tv-terminal.md` via pidag

---

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
