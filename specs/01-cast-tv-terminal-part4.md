# Spec: cast-tv-terminal — Part 4/4

**Parent-Spec**: `01-cast-tv-terminal.md`
**Part**: 4 of 4
**Covers**: R8–R11 (no unwrap, dep discipline, hardware encode, safe degrade),
the full 10-test TDD contract, and the parent's fail-closed final sweep
**Status**: SPECIFIED — AMENDED 2026-08-16 (TDD now lists the full parent
contract so this part confirms every test exists and passes; the no-unwrap
sweep made fail-closed so a missing module dir fails rather than passing
vacuously).

## Overview

Display the live screen content of the `herdr` terminal multiplexer (a tmux-
compatible mouse-first multiplexer) as a full app window on a TV via a Google
Chromecast on the same LAN. Uses the Default Media Receiver (CC1AD845) + HLS:
capture the pane over a tmux-style pipe/socket bridge, render the terminal grid
to video with alacritty_terminal/vte + GStreamer (H.264, 1080p/60), and cast the
low-latency HLS URL with rust_cast. Avoids a custom Cast receiver app and Google
Cast registration.

**Part 4 is the closing sweep**: confirm the whole suite, every module file, and
the R8 no-unwrap discipline. No new modules.

---

## TDD Contract (full parent contract — all must be present and passing)

| Test Name | Given | Expects |
|-----------|-------|---------|
| `test_vte_parses_ansi_into_grid` | ANSI/VT sequence with cursor+colors | grid with correct chars + Cell colors |
| `test_first_frame_is_full` | parser first bytes | `ScreenFrame.full == true`, all cells set |
| `test_subsequent_frames_are_diff` | two updates to one region | 2nd frame carries only changed cells |
| `test_rasterize_grid_to_buffer` | small grid with known colors | RGB buffer correct size + pixels |
| `test_hls_playlist_has_cors` | HLS server response | `Access-Control-Allow-Origin` present |
| `test_served_segment_bytes` | GET a segment | HTTP 200 + non-empty body |
| `test_cast_load_url_builds_media_load` | device + HLS URL | Cast v2 `media/load` payload (HLS) |
| `test_capture_bridge_feeds_bytes_to_vte` | fake pane emitting bytes | emu screen advances |
| `test_sender_reports_unreachable` | discovery finds no device | clear error (no hang) |
| `test_no_production_unwrap` | walk of `src/` production files | no `.unwrap()`/`.expect()` outside tests |

## Implementation Notes

- If any of the ten tests is missing or failing, that is the work of this part:
  add the test and the code that satisfies it. Do not weaken an existing test to
  make it pass.
- `test_no_production_unwrap` walks `src/capture`, `src/emu`, `src/render`,
  `src/encode`, `src/serve`, `src/cast` (relative to `CARGO_MANIFEST_DIR`) and
  fails if a production (non-test, non-comment) line contains `.unwrap()` or
  `.expect()`.
- No new dependencies. No new public API beyond the parent spec.

## Exit Criteria

- [ ] `cargo test --test cast_tv_tests 2>&1 | grep -q "test result: ok"`
- [ ] `for f in src/capture/bridge.rs src/emu/term.rs src/render/raster.rs src/encode/pipe.rs src/serve/server.rs src/cast/sender.rs; do [ -f "$f" ] || exit 1; done`
- [ ] `grep -q "media/load" src/cast/sender.rs`
- [ ] `for d in src/capture src/emu src/render src/encode src/serve src/cast; do [ -d "$d" ] || exit 1; done && ! grep -rE '\.unwrap\(\)|\.expect\(' src/capture src/emu src/render src/encode src/serve src/cast 2>/dev/null | grep -v '//' | grep -v '#\[cfg\(test\)\]' | grep -v test`

## Guardrails

- Do not run `cargo`, `rustc`, `clippy` outside the TDD cycle steps
- Do not add public API surface not specified in Requirements
- Do not use `.unwrap()`, `.expect()`, or `panic!()` in production code paths
- Do not modify files outside the project root
- Do not add dependencies to `Cargo.toml` without explicit approval
- **Approved dependencies**: `alacritty_terminal`, `rust_cast`, `gstreamer`,
  `gstreamer-video`, `tiny-skia`, `tokio`, `axum` (or `hyper`), `serde`,
  `serde_json`, `thiserror`
- **PIVOT GUARDRAIL**: do NOT build a custom Cast receiver, register with Google
  Cast, or use WebRTC for the first milestone. If `rust_cast` cannot `media_load`
  HLS onto the device, STOP and report for an explicit Option-2 decision.

On any ambiguity, stop and report back, do not guess.

---
