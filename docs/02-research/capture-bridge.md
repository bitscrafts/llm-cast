# Capture bridge: tmux/zellij-style pane output (research)

Status: DRAFT (2026-08-07). Source: tmux(1) man page (man7.org).
Context: herdr is tmux/zellij-compatible (mouse-first multiplexer, detach/attach,
session state from `muxer-client.md`). Even without a live herdr instance, the
tmux capture idioms give us the concrete mechanisms to tee a pane into `vte`.

## The three capture primitives in tmux
1. **`pipe-pane [-IOo] [-t target-pane] [shell-command]`** — *"Pipe output sent by
   the program in target-pane to a shell command or vice versa."*
   ✅ THIS is the primary bridge: `tmux pipe-pane -t <pane> <our-rust-bridge>` gives
   us a **live byte stream** of the pane's output. Feed that into `vte` → grid.
   (Use `-o` to also keep the pane attached to the normal display.)
2. **`capture-pane [-p] [-t pane] [-S start] [-E end]`** — snapshot the pane's
   current contents to stdout (`-p`). Good for a fast "latest frame" or recovery
   after attach (fresh client gets a full redraw), not for continuous streaming.
3. **Control mode** (client socket): tmux server runs over a UNIX socket
   (`-S socket-path`). A client in **control/`-CC` mode** speaks a machine-
   readable protocol; the socket enables attach/reattach and pane events.

## Zellij counterpart
Zellij is a server/client multiplexer with a **plugin/API system** and its own
protocol; it offers WS/plugin-based access rather than the raw tmux `pipe-pane`
idiom. Documentation confirms a Plugins + Layouts API exists. (Relevant if herdr
turns out to be zellij-based rather than tmux-based.)

## Implication for our pipeline (Rust capture bridge)
```
herdr/tmux pane --pipe-pane--> [bridge reads pane bytes]
   -> alacritty_terminal/vte (grid) -> rasterize -> GStreamer H.264 -> HLS -> cast
```
- The bridge is a small Rust binary that `stdin`/socket-receives pane output and
  feeds `vte`; on the first frame it re-emits a full grid (so a fresh Cast client
  shows a complete screen, not a blank), then only diffs change.
- If herdr exposes a websocket/session-socket (like tmux control mode or zellij),
  attach the same way: read socket → vte.
- Fallback if only `capture-pane` exists: poll snapshots; coarser but still
  low-cost for a low-motion terminal.

## Open / to verify with a live herdr
- Exact herdr equivalent of `pipe-pane`/attach (command vs socket vs plugin API).
- Whether herdr auto-spawns a pty we can own directly. Keep the tmux idiom as the
  best-known capture contract to implement against.
