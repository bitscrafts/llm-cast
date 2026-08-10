# HANDOFF — INCOMPLETE (runtime iteration-budget handoff, 2026-08-10)

**Status**: YELLOW — TDD implementation NOT yet written; API verification COMPLETE.
**Date**: 2026-08-10 (mid-session, tool-iteration budget reached → graceful handoff)

---

## What was DONE this session

- Verified every API surface the implementation needs against vendored crate sources
  (alacritty_terminal 0.24.2, rust_cast 0.17.0, vte 0.13.1, tiny-skia 0.11.4, axum 0.7.9).
  Full detail in **`docs/02-research/api-intel-v2-addendum.md`** (read first).
- Key corrections/confirmations vs api-intel-v1:
  - **`Line(0)` = top visible row; `Line(rows-1)` = bottom** (Storage::compute_index
    verified). Use nested `grid[Line(r as i32)][Column(c)]` loops — NOT `display_iter()`
    (its order starts at bottom row).
  - `Config::default()` has `scrolling_history: 10000` → set `scrolling_history: 0` so the
    visible window never drifts.
  - **`impl Dimensions for (usize, usize)` is `#[cfg(test)]`-gated** → write own
    `struct TermSize(usize, usize)` implementing `Dimensions`.
  - rust_cast: `Media` has **no constructor** — struct literal
    `{ content_id, stream_type: StreamType::Live, content_type: "application/x-mpegurl", metadata: None, duration: None }`;
    `device.receiver.launch_app(&CastDeviceApp::DefaultMediaReceiver)` → `Application`
    (use `.transport_id` as media-channel destination, `.session_id` as session);
    `device.media.load(dest, session_id, &media)` sends the `LOAD`/`media/load` payload.
- Committed: `docs/02-research/api-intel-v2-addendum.md`.

## What REMAINS (next agent — start here)

1. Write implementation (NO code written yet). Plan in
   `specs/01-cast-tv-terminal.md`; module tree in spec §Module Structure. All 10 TDD
   tests must pass with **default features** (no `--features cast,gstreamer`).
2. **Feature-gating decision (already made)**: `src/cast/` and `src/encode/` are
   `#[cfg(feature = ...)]` in lib.rs; the cast payload test builds OUR OWN serializable
   `LoadPayload` struct (feature-independent) so `test_cast_load_url_builds_media_load`
   runs in the default build. rust_cast/gstreamer are runtime/target-host only.
3. `src/emu/` (term.rs: TermSize + Term wrapper + Processor feed; ScreenFrame/Cell diff
   via HashMap of last cells; first frame `full: true`) → `src/render/` (raster.rs with
   `include!` font8x8 — `src/render/font.rs` already generated from `src/font8x8_basic.h`)
   → `src/capture/` (bridge.rs: fake-pane bytes → emu feed) → `src/serve/` (axum HLS:
   `/live.m3u8` playlist + `/seg/N.ts` segments with `Access-Control-Allow-Origin: *`;
   tower-http CORS dep already in Cargo.toml) → `src/cast/` (sender.rs must contain the
   literal `media/load` for exit-criterion grep) → `src/encode/` (pipe.rs must contain
   `h264` for exit-criterion grep; gated plan-only).
4. `tests/cast_tv_tests.rs` — 10 tests per TDD contract (already declared `[[test]]`
   in Cargo.toml). `test_sender_reports_unreachable`: discovery must return error quickly
   (timeout), no hang.
5. TDD cycle: `cargo test --test cast_tv_tests` → fix → exit criteria →
   `bash /root/.pi/agent/skills/quality-gate/run.sh .` (cargo only inside the cycle).
6. Exit criteria: `grep "media/load" src/cast/sender.rs`,
   `grep "Access-Control-Allow-Origin" src/serve/server.rs`,
   `grep -i h264 src/encode/pipe.rs`, no unwrap/expect in src/{capture,emu,render,encode,serve,cast}.
7. Milestone-1 smoke (operator): rust_cast media_load of HLS URL onto device.

## Key files
- `docs/02-research/api-intel-v2-addendum.md` — VERIFIED API reference (read first)
- `specs/01-cast-tv-terminal.md` — spec + TDD contract + exit criteria
- `src/render/font.rs` + `src/font8x8_basic.h` — font ready for rasterizer
- `Cargo.toml` — deps declared (axum 0.7, tower-http cors, bytes, base64, log already
  present; gstreamer/rust_cast optional)

## Memory keys (session)
- `chromecast-tv-mirror/implementation/api-intel-v1` (prior, 0.8)
- NEW: store `chromecast-tv-mirror/implementation/api-intel-v2` (importance 0.8) =
  line-indexing + rust_cast Media/load + feature-gating decision. (Not yet stored —
  next agent MUST run `agent-memory store` per protocol.)
