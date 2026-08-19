# llm-cast

**Put a live terminal on your TV and watch an LLM work in real time.**

`llm-cast` captures a terminal multiplexer session (herdr/tmux), renders it to
video, encodes it to H.264/AAC, serves it as low-latency HLS, and casts it onto
a **Google Chromecast** (Default Media Receiver). An MCP server exposes the
whole thing as tools an AI agent can call.

```
capture (pipe-pane) → emu (vte grid) → rasterize (RGBA) → encode (H.264/AAC)
    → serve (HLS) → cast (rust_cast → Google Chromecast DMR)
```

## What it does

- **Cast a live terminal to a Google Chromecast** — mirror any herdr/tmux
  session onto the TV.
- **Watch an LLM work** — follow an agent's output in real time (~1–2 s HLS
  delay).
- **MCP server** — 10 tools an agent can call to control the TV
  (`cast_url`, `cast_text`, `mirror_session`, `set_config`, `pipeline_status`,
  and more).
- **Real audio** — feed a WAV/audio source into the HLS stream, or keep the
  silent-AAC default the Chromecast DMR requires.

## Quick start

```bash
# Build the full stack (needs GStreamer + system deps)
cargo build --release --features cast,mdns,gstreamer

# Run the MCP server (stdio)
mcp-server

# Cast a live terminal pane to the Chromecast
mirror --source <pipe-pane-file> \
       --audio-source 'filesrc location=/tmp/song.mp3 ! decodebin ! audioconvert ! audioresample' \
       --bind 0.0.0.0:8080 --outdir /tmp/hls \
       --url-base http://<LAN-IP>:8080/live.m3u8 \
       --device 10.10.10.208

# Smoke-test a Chromecast device
castctl --ping 10.10.10.208
```

## MCP endpoints

| Tool | Description |
|------|-------------|
| `cast_url` | Cast a media URL (HLS, MP4, image) to the TV |
| `cast_text` | Show agent text on the TV |
| `run_command` | Run a shell command; output appears on the TV |
| `set_font_size` | Adjust the display font size |
| `pipeline_status` | Report the live pipeline state as JSON |
| `restore` | Return the TV to the cycling view |
| `mirror_session` | Mirror a running terminal session on the TV |
| `set_config` | Change display settings at runtime |
| `save_profile` / `load_profile` | Save/load display profiles |

## Features

- `gstreamer` — real H.264/AAC encoding
- `cast` — rust_cast device session (media load onto Chromecast)
- `mdns` — mDNS discovery of cast devices on the LAN

## Configuration

All runtime config comes from the environment (`CAST_DEVICE`, `MUX`,
`TV_RESOLUTION`, `TV_TERMINAL`, `HLS_DIR`, ...). See `INDEX.md` for the full
table.

## Development

Spec-driven: each feature is a numbered spec in `specs/`. Implemented via
pi-orchestration (planner authors specs, a local workhorse implements them in
isolated git worktrees, the orchestrator reviews and commits).

See **`INDEX.md`** for the full app overview, architecture, and the
LLM-activity / delay details.
