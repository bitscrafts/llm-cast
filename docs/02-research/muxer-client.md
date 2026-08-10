# Muxer client: herdr terminal model (research)

Status: DRAFT (2026-08-07). Sources: herdr.dev/docs (loaded), herdr docs text.

## What herdr is
Herdr is a **terminal multiplexer**, positioned as "mouse-first" but
tmux/zellij-compatible:
- **"Coming from tmux or zellij? You already know the model. Prefix ctrl+b, panes
  persist, detach and reattach the way you expect."**
- Mouse-first UX: click panes, drag borders, split/switch from right-click menus.
- Session state concepts: detach, restart/restore, **pane history replay**, native
  agent resume, live handoff; "direct attach".

## Implications for capturing content to the TV
- Herdr manages **terminal sessions/panes** in a tmux-like client-server model.
  It runs jobs inside PTYs; panes are terminal screens.
- Because it looks/behaves like tmux/zellij, we have a strong prior for capture:
  1. **Session/attach API**: if herdr exposes a "attach" (like `herdr attach` or
     an SSH-style control socket), we can attach to a pane, tee its output
     through `vte`, and render the grid — same pipeline as `rust-first-stack`.
  2. **Pane history / replay**: "pane history replay" suggests sessions retain
     screen content that could be re-emitted on attach — helpful for a client
     that just wants the latest frame.
  3. Fallback: run a pane/command whose stdout/pty we control entirely, so the
     capture bridge owns the pty from the start.
- **Still to confirm (needs a real herdr instance):** exact attach command /
  socket / websocket, whether it exposes a web/screenshare endpoint, and how to
  subscribe to a single pane's byte stream. This is the top implementation-input
  research item — I can only get so far on docs.

## Recommended capture stance (Rust)
- Prefer a **replay/attach feed** of the pane → `vte` grid, in Rust.
- Do NOT video-scrape a window; do NOT rely on `capture-pane`-style text snapshots
  (lose colors/position). Use a full grid render.

## Doc/memory trail
- Insight: `chromecast-tv-mirror/research/herdr-terminal-model` (this note).
