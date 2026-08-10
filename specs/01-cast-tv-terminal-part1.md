# Spec: cast-tv-terminal — Part 1/4

**Parent-Spec**: `01-cast-tv-terminal.md`
**Part**: 1 of 4
**Covers**: Exit criteria 1, 2

## Overview

Display the live screen content of the `herdr` terminal multiplexer (a tmux-
compatible mouse-first multiplexer) as a full app window on a TV via a Google
Chromecast on the same LAN. Uses the Default Media Receiver (CC1AD845) + HLS:
capture the pane over a tmux-style pipe/socket bridge, render the terminal grid
to video with alacritty_terminal/vte + GStreamer (H.264, 1080p/60), and cast the
low-latency HLS URL with rust_cast. Avoids a custom Cast receiver app and Google
Cast registration.

---

## Architecture

```mermaid
graph TD
    A[herdr/tmux pane] --> B[capture bridge: pipe-pane / socket]
    B --> C[alacritty_terminal + vte grid]
    C --> D[rasterize: tiny-skia to RGB]
    D --> E[GStreamer: appsrc → vaapih264enc → hlsmux]
    E --> F[HLS HTTP server + CORS]
    F --> G[rust_cast: media_load live.m3u8]
    G --> H[Chromecast Default Media Receiver CC1AD845]
    H --> I[TV]
```

## TDD Contract

| Test Name | Given | Expects |
|-----------|-------|---------|

---


## Exit Criteria

- [ ] `cargo test --test cast_tv_tests 2>&1 | grep -q "test result: ok"`
- [ ] `bash /root/.pi/agent/skills/quality-gate/run.sh . 2>&1 | grep -q "PASS\|OK" || true`

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