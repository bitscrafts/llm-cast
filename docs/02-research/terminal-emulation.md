# Terminal emulation on the host (research)

Status: DRAFT (2026-08-07). Source: `alacritty_terminal` 0.26.0 docs (loaded),
DuckDuckGo recon of terminal emulators. Rust-first.

## Context from the user
- The muxer (`outer/herdr`) **just uses a terminal** — i.e. it drives a PTY /
  a terminal emulator. There is no bespoke "video/display" surface to scrape
  directly; the content we want on the TV is **terminal screen content**.
- Same network; goal shows an **app window** on the TV; **no strict latency**
  constraint.

## The pivotal Rust crate: `alacritty_terminal`
`alacritty_terminal` **0.26.0** (active, 2026-07, Apache-2.0) is the emulation
core of the Alacritty GPU terminal, **without** the GUI. It gives us exactly
what we need to own terminal rendering in Rust:
- **`vte`** module — escape-sequence parser (ANSI/VT) that turns a PTY byte
  stream into screen mutations.
- **`pty`** module — PTY creation (+ `rustix-openpty` dep).
- **`term`** module — the `Terminal` screen model.
- **`grid`** module — the cell grid (each cell: char + style/color attributes).
- **`event`** — `EventListener` extension points to observe changes/diffs.
- Other deps visible: `unicode-width`, `bitflags`, `signal-hook`, `vte ^0.15`.

Why this matters: rather than video-capture a window (fragile), we can **parse
the PTY stream into a text/cell grid** with `vte`, keep a full grid fully in
Rust, and render that grid to pixels ourselves — then feed the pixel buffer to
our encoder (GStreamer → H.264) without ever scraping the X11/Wayland window.

### Two capture models (depending on how herdr gives us the terminal)
1. **We own the PTY**: if herdr runs as a client of a muxer we can start, we
   spawn the pty and forward its output both to the real terminal AND into
   `alacritty_terminal`/`vte` for rendering. Deterministic, no window scraping.
2. **Attach to an existing pty/session**: if herdr exposes an SSH/attach or
   control socket, we attach and tee its output through the same vte pipeline.

Either way the rendering side is identical: **grid → pixels → encoder**.

## Rendering grid → video
- A terminal grid is low-motion text; we can render at modest resolution
  (e.g. 1920x1080 or scaled from e.g. 120x40 cols) at 10-30 fps.
- Render options in Rust: `skia-safe`/`tiny-skia`, `softbuffer`/`winit` (headless
  swapchain), or Cairo. Simplest robust path: rasterize the grid into an ARGB
  buffer ourselves, then `mpeg2`/GStreamer `appsrc` → vaapih264 → HLS.
- Because latency is not strict, we don't need WebRTC; low-latency HLS (1-3s) or
  even plain HLS is acceptable — aligns with Architecture A (Default Media
  Receiver + HLS) from `rust-first-stack.md`.
- Color/fidelity: render with the terminal's 256-color/truecolor attrs onto a
  chosen palette so the TV view is readable.

## Alternatives (evaluated, mostly rejected for this project)
- **libvterm (C)** — battle-tested terminal emulation in C, ffi-able, but we
  prefer pure-Rust + `vte`.
- **Video-capture the emulator window** (X11 `xdotool`/X or pipewire screencast)
  — works but is fragile (window must be visible), ties to a display server, and
  wastes encode on GUI chrome. Prefer text-grid → pixels.
- Plain `script`/`tmux capture-pane` as raw text — loses colors/positions for
  pixel-faithful rendering; a proper emulator model (grid) is more robust.

## Next research forks (needed to finalize)
1. Exactly how herdr attaches/owns its terminal: does it (a) spawn a PTY and
   manage it, (b) attach via SSH/control socket, or (c) fork a child + read its
   stdout? This picks capture-model 1 vs 2.
2. GStreamer-rs encode pipeline details (vaapih264enc → hlsink2; low-latency hls
   params, segment size) for our low-motion text source.
3. Best Rust grid→pixels rasterizer (tiny-skia vs cairo vs softbuffer) for
   crisp readable text at 1080p.
4. Confirm `vte` diffing: we only re-encode changed cells → cheaper encode.

## Memory/doc trail
Insight stored: `chromecast-tv-mirror/research/terminal-emulation` (this note).
