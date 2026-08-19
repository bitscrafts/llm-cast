# cast-tv-terminal

**Display a terminal multiplexer session on your TV via Chromecast.**

`cast-tv-terminal` captures a live terminal pane (herdr/tmux `pipe-pane`),
renders it to a video frame, encodes it to H.264, serves it as low-latency
HLS, and casts it onto a **Google Chromecast** (Default Media Receiver). An
MCP server exposes the whole thing as tools an AI agent can call.

- **Crate**: `cast-tv-terminal` v0.1.0
- **Language**: Rust (edition 2021)
- **License**: (add before publishing)
- **Repository**: `llm-cast` (GitHub, bitscrafts)

---

## What it does

```
capture (pipe-pane) → emu (vte grid) → rasterize (RGBA) → encode (H.264/AAC)
    → serve (HLS) → cast (rust_cast → Chromecast DMR)
```

A terminal multiplexer session (herdr or tmux) is piped into the app, parsed
into a screen grid, rasterized to pixels, encoded with GStreamer, served as an
HLS stream, and loaded onto a Chromecast. The result: **your terminal on the
TV**, with real audio support.

---

## Binaries

| Binary | Purpose |
|--------|---------|
| `mirror` | Run the whole cast path: `--source <pipe-pane> [--audio-source <fragment>] [--bind A:P] [--size WxH] [--outdir DIR] [--encoder x264\|vaapi] [--device IP] [--url-base URL] [--no-cast]` |
| `castctl` | Operator smoke-test: `castctl [--ping] [--image] [--type CONTENT-TYPE] <device-ip> <url>` |
| `mcp-server` | MCP-over-stdio server exposing the TV as agent tools |

---

## Features (Cargo)

| Feature | Enables |
|---------|---------|
| *(default)* | Core pipeline + MCP server (no codec, no cast) |
| `gstreamer` | Real H.264/AAC encoding via GStreamer |
| `cast` | rust_cast device session (media load onto Chromecast) |
| `mdns` | mDNS discovery of cast devices on the LAN |

---

## MCP Endpoints (mcp-server)

The MCP server exposes the TV as tools an agent can call. All tools are always
listed; feature-gated ones return a clear error when built without the
feature (never crash).

| Tool | Description | Feature |
|------|-------------|---------|
| `cast_url` | Cast a media URL (HLS playlist, MP4, or image) to the TV via the configured Chromecast | `cast` |
| `cast_text` | Show agent text on the TV in the dedicated agent window | — |
| `run_command` | Run a shell command verbatim in the agent window; its output appears on the TV | — |
| `set_font_size` | Relaunch the display xterm with a new font size in points (6..=32) | — |
| `pipeline_status` | Report the live TV pipeline state as one JSON text block | — |
| `restore` | Restore the TV to the cycling view; restart the cycle loop unless disabled | — |
| `mirror_session` | Mirror a running terminal session on the TV via the display xterm | — |
| `set_config` | Change display settings (resolution/terminal/margin/geometry) at runtime | — |
| `save_profile` | Save the current display settings as a named profile | — |
| `load_profile` | Load a saved display profile and apply it (relaunches a running display) | — |

---

## Configuration (environment)

All runtime config comes from the environment with documented defaults.

| Env var | Default | Meaning |
|---------|---------|---------|
| `MUX` | `herdr` | Multiplexer: `herdr` or `tmux` |
| `MUX_SESSION` | `tv-demo` | Multiplexer session name |
| `MUX_SOCKET` | `$HOME/.config/herdr/sessions/tv-demo/herdr.sock` | herdr socket path |
| `MUX_WORKSPACE` | `w1` | herdr workspace id |
| `MUX_AGENT_LABEL` | `agent` | Window label the agent tools use |
| `MUX_CYCLE_LABELS` | `1,watch` | Comma-separated tab labels shown by the cycle |
| `MUX_FOCUS_SECS` | `10` | Seconds each tab is focused by the cycle |
| `CAST_DEVICE` | `10.10.10.208` | Chromecast host or IP |
| `HLS_DIR` | `/tmp/m2/xhls` | Directory of the HLS segments served to the TV |
| `CYCLE_PID_FILE` | `/tmp/m2/tv_cycle.pid` | PID file of the running cycle loop |
| `X_DISPLAY` | `:99` | Framebuffer display |
| `XTERM_GEOMETRY` | *(computed)* | Legacy verbatim xterm geometry override |
| `TV_RESOLUTION` | `1280x720` | Display frame the pipeline renders at (`WxH`) |
| `TV_TERMINAL` | `116x34` | Logical display terminal size (`CxR`) |
| `TV_MARGIN` | `0.10` | Symmetric inset fraction of each frame edge kept clear |
| `PROFILES_DIR` | `$HOME/.config/chromecast-tv-mirror/profiles` | Where `save_profile`/`load_profile` read/write |

---

## Architecture

```
src/
├── lib.rs            # module roots
├── bin/
│   ├── mirror.rs     # full cast path operator binary
│   ├── castctl.rs    # device smoke-test binary
│   └── mcp-server.rs # MCP-over-stdio server
├── capture/          # R1: pipe-pane byte source + bridge
├── emu/              # R2: vte terminal emulation → cell grid
├── render/           # R3: grid → RGBA raster (font + damage)
├── encode/           # R4: RGBA → H.264/AAC → HLS (GStreamer)
├── serve/            # R5: low-latency HLS HTTP server (axum)
├── cast/             # R6: rust_cast device session + mDNS discovery
├── mux/              # herdr/tmux multiplexer control
├── mcp/              # MCP server (tools, config, display, runner)
├── pipeline/         # capture→emu→rasterize→encode coordinator
└── damage.rs         # R7: changed-cell tracking
```

---

## Quick start

```bash
# Build the full stack (needs GStreamer + system deps)
cargo build --release --features cast,mdns,gstreamer

# Run the MCP server (stdio)
mcp-server

# Cast a live terminal pane to the TV
mirror --source <pipe-pane-file> \
       --audio-source 'filesrc location=/tmp/song.mp3 ! decodebin ! audioconvert ! audioresample' \
       --bind 0.0.0.0:8080 --outdir /tmp/hls \
       --url-base http://<LAN-IP>:8080/live.m3u8 \
       --device 10.10.10.208

# Smoke-test a device
castctl --ping 10.10.10.208
```

---

## Setting up the MCP server

The MCP server exposes the TV as tools an agent (Claude Code, Codex, Hermes,
...) can call. It runs over **stdio** — the agent launches it as a subprocess
and talks JSON-RPC over stdin/stdout.

### 1. Build the server

```bash
cargo build --release --features cast,mdns,gstreamer
cp target/release/mcp-server ~/.local/bin/mcp-server
```

### 2. Bring up the TV display stack

The TV runs its own herdr session (`tv-demo`) with the project's herdr config.
The display pipeline (Xvfb `:99`, ffmpeg x11grab→HLS, hls_server) must be
running, then:

```bash
scripts/tv-demo-up.sh   # starts the tv-demo herdr server + display xterm + panes
```

### 3. Register the server with your agent

The server is configured entirely via environment variables (see the config
table above). A ready-made `.mcp.json` is in the repo root — point it at your
installed binary and adjust `CAST_DEVICE`/`MUX_SESSION` to your setup.

**Claude Code** — add to `~/.claude.json` (or the project's `.mcp.json`):

```json
{
  "mcpServers": {
    "chromecast": {
      "type": "stdio",
      "command": "/root/.local/bin/mcp-server",
      "args": [],
      "env": {
        "CAST_DEVICE": "10.10.10.208",
        "MUX": "herdr",
        "MUX_SESSION": "tv-demo",
        "MUX_SOCKET": "/root/.config/herdr/sessions/tv-demo/herdr.sock",
        "TV_RESOLUTION": "1280x720",
        "TV_TERMINAL": "165x50",
        "TV_MARGIN": "0.05"
      }
    }
  }
}
```

**Codex** — add to `~/.codex/config.toml`:

```toml
[mcp_servers.chromecast]
command = "/root/.local/bin/mcp-server"
env = { CAST_DEVICE = "10.10.10.208", MUX = "herdr", MUX_SESSION = "tv-demo",
        MUX_SOCKET = "/root/.config/herdr/sessions/tv-demo/herdr.sock",
        TV_RESOLUTION = "1280x720", TV_TERMINAL = "165x50", TV_MARGIN = "0.05" }
```

**Hermes** — `hermes mcp add chromecast -- /root/.local/bin/mcp-server` (or
edit `~/.hermes/config.yaml`).

### 4. Use it

Once registered, the agent sees the 10 tools. For example, in Claude Code or
Codex, just ask:

> "Mirror the `tv-demo` session on the TV" → calls `mirror_session`
> "Show 'hello' on the TV" → calls `cast_text`
> "Cast this HLS URL to the TV" → calls `cast_url`
> "What's on the TV right now?" → calls `pipeline_status`

The agent calls the tools automatically; you don't type JSON-RPC yourself.

---

## Casting a terminal / following LLM activity

The core use case: **put a live terminal on the TV and watch an LLM work in
real time.** You mirror a herdr/tmux session (e.g. the one running an agent),
and the TV shows its output as it happens.

### The two paths

**1. `mirror_session` (MCP tool) — the display xterm path.**
`mirror_session <session>` kills the display xterm and respawns it attached to
the given herdr/tmux session, sized to the effective display settings. The
xterm runs on the framebuffer display (`X_DISPLAY`, default `:99`); the
pipeline captures that frame, encodes it, and casts it. This is the
"watch a live session" path — the agent's terminal appears on the TV.

**2. `mirror` (binary) — the pipe-pane path.**
`mirror --source <pipe-pane-file>` reads a herdr/tmux `pipe-pane` output file
directly (no xterm), renders it, and casts it. This is the "stream a specific
pane" path.

### The delay: ~1–2 seconds

The pipeline is **not** real-time video; it's a low-latency HLS stream, and
the end-to-end delay is **roughly 1–2 seconds**. The contributors:

| Stage | Cost |
|-------|------|
| Capture poll (`tick_ms=10`) | ~10 ms |
| Rasterize + encode (10 fps, `FPS=10`) | ~100 ms/frame |
| HLS segment (`target-duration=1`) | up to 1 s |
| Chromecast fetch + buffer + play | ~0.5–1 s |

So when an LLM prints a line, it appears on the TV **about 1–2 seconds later**.
This is fine for *watching* an agent work (you see the output as it streams),
but it is **not** suitable for interactive typing or anything needing
sub-second feedback. The trade-off is inherent to HLS + Chromecast: the DMR
buffers a segment before playing, which is what makes the stream reliable.

### Following an LLM session

1. Start your agent in a herdr/tmux session (e.g. `tv-demo`).
2. Call `mirror_session` with that session name — the TV shows the agent's
   terminal.
3. Watch the output stream in ~1–2 s. Use `set_font_size` to adjust legibility,
   `set_config` to change resolution/terminal size, and `restore` to return to
   the cycling view when done.

The `pipeline_status` tool reports the live pipeline state (device, session,
frame size) as JSON, so you can confirm what's on the TV.

---

## Development

- **Spec-driven**: each feature is a numbered spec in `specs/` (01 master,
  02 damage, 03 mcp-server, 04 mDNS, 05 castctl, 06 real audio).
- **Workflow**: pi-orchestration — a planner (glm-5.2) authors specs, a local
  workhorse (laguna) implements them in isolated git worktrees, the orchestrator
  reviews and commits.
- **Tests**: `cargo test` (unit + integration), `cargo clippy -- -D warnings`,
  `cargo fmt -- --check`. GStreamer/cast tests are feature-gated.
- **Verification**: `_tmp/audio_verify.sh` proves real audio lands in the HLS
  output (in-container, no TV needed).

---

## Status

- **spec-01** master pipeline (6 parts): ✅
- **spec-02** damage tracker: ✅
- **spec-03** MCP server (3 parts): ✅
- **spec-04** mDNS discovery: ✅
- **spec-05** castctl `--ping`: ✅
- **spec-06** real audio (3 parts): ✅
