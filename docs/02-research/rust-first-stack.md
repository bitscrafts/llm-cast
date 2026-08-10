# Rust-first implementation stack (research)

Status: DRAFT (2026-08-07). Rust-first: every subsystem is evaluated for an
existing Rust crate before considering C/C++/other. Sources: crates.io (live
queries), rust-cast GitHub README, prior docs in this folder.

## The pivotal finding: we likely do NOT need a custom receiver app
The `chromecast` crate (`rust_cast` / `rust-cast`, GitHub `tsirysndr/rust-cast`,
derived from Chromium Open Screen) is a Rust **Cast v2 sender**. Its example
shows it can, over the Cast protocol:
- Discover + connect to a Chromecast on the LAN.
- Query running apps (sees the built-in **Default Media Receiver** `CC1AD845`).
- **Launch an app on the device** and send media-load messages.

⟹ Architecture A (lowest effort, no Google registration):
host runs a Rust HTTP/HLS server that **pushes an HLS live-stream URL to the
Default Media Receiver** (CC1AD845) via rust_cast's Cast v2 media-load. The
built-in receiver plays the HLS stream. No custom receiver app, no Google
console registration, no CORS-on-receiver concern (we control the HLS server and
can add CORS anyway).

Caveat to verify: rust_cast is **old (v0.18.2, 2023-03-30)** — confirm the
media-load/play message path works against a current Chromecast firmware. If the
sender path is broken, Architecture B (custom receiver) is the fallback.

## Candidate crate map (all verified to exist on crates.io)
| Concern | Crate | Maturity | Notes |
|---|---|---|---|
| Cast v2 sender (discover+media-load) | `chromecast`/`rust_cast` | stale (2023) | THE key crate to evaluate first |
| Hardware video encode (H.264) | `gstreamer` | mature, active (9.4M dl) | VA-API / nvcodec / qsv via gst plugin |
| Sub-second transport | `webrtc` | mature (5.4M dl) | `webrtc-rs`, for LL-HLS alternative |
| HLS manifest gen | `hls_m3u8` | uses (0.7.0) | manifest only; media muxing not included |
| MPEG-TS muxing | `mpegts` | stale (2018) | avoid; prefer GStreamer's muxer |
| (not cast) | `c2pa` | — | C2PA content-photo standard; NOT cast — ignore |

Note `vaapi`/`rtsp` had no direct crates on crates.io by those exact names;
routing via GStreamer (which bundles vaapi/rtsp/soup elements) avoids pulling
thin/unofficial crates.

## Two concrete architectures (Rust)
- **A. Default Media Receiver + HLS (preferred)**: terminal text/canvas →
  [GStreamer-rs encode to H.264, small low-latency HLS with a few segments] →
  rust-cast `media_load` of `http://<host>:PORT/live.m3u8` onto CC1AD845.
  Latency: low-latency HLS ~1-3s (use small segments / partial segments).
- **B. Custom Web Receiver + WebRTC/LL-HLS**: more control/latency (<1s via
  WebRTC) but requires a custom receiver app (Google Cast SDK registration) and
  more build effort. Reserve for if (A) latency/registration can't be avoided.

## Where the Rust terminal rendering comes from
The muxer (`outer/herd`) must be captured. Whichever capture:
- If the muxer exposes a **web/HTML view** or a way to render its pane(s) to
  pixels: feed GStreamer a raw/video source directly.
- If only a **pty**: render to a framebuffer/window ourselves (Rust: `ratatui`
  or a terminal-to-ATSC text model is overkill; simplest is to screenshot a
  GPU/VirtualFB backend and pipe to GStreamer). **Still TBD — see muxer-client.md.**

## Non-goals / notes
- No custom receiver for the first slice (architecture A avoids it).
- `mpegts` and direct MPEG-TS muxing avoided; GStreamer owns muxing.
- OS-level capture (X11/wayland screencapture crates) is a separate to-evaluate
  bucket and depends on what the muxer renders to.

## Next research
1. Confirm rust_cast media-load actually streams HLS to a current CC firmware
   (top risk).
2. Confirm muxer capture surface (web? pty? X/GPU?) -> decides the render+encode
   input for GStreamer.
3. Chromecast model/gen + LAN/Wi-Fi topology + latency budget.
4. GStreamer-rs low-latency HLS encode pipeline specifics (avdeinterlace, x264
   vs vaapih264enc, hlsink2 params).
