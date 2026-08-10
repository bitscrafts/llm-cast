# Proposed solution architectures (discussion draft)

Status: DRAFT (2026-08-07), prepared for a discussion of the most versatile +
feasible option. Depends on all `02-research/` notes.
Goal: show an `outer/herd` terminal session as an **app window on a TV** via a
Google Chromecast on the same LAN. No strict latency. Rust-first.

## Locked-in facts (research)
- Herdr uses a **plain terminal** → we can capture it as a PTY/screen grid.
- Same LAN; host + Chromecast reachable; app-window UX target; latency OK.
- Chromecast = headless Cast receiver (Chrome-like webview). To show live content
  we either (A) push a media/HTML stream to the **built-in Default Media Receiver**
  (CC1AD845), or (B) run a **custom receiver app** (needs Google Cast registration).
- **H.264 is the universal codec**; terminal text is low-motion → tiny bitrate.
- Rust building blocks: `alacritty_terminal`/`vte` (parse terminal→grid),
  `gstreamer` (HW H.264 encode), `rust_cast` (Cast v2 sender), `webrtc` (alt),
  `hls_m3u8` (manifest).

## Candidate architectures

### Option 1 — Default Media Receiver + HLS (LOWEST EFFORT)  ★ recommended
Pipeline:
```
herdr PTY ──> alacritty_terminal(vte) grid ──> rasterize (tiny-skia) RGB
              ──> GStreamer appsrc → vaapih264enc → hlsmux → low-latency HLS
              ──> rust_cast media-load http://host/live.m3u8 → CC1AD845
```
- No custom receiver, no Google registration, no CORS issue.
- Latency ~1-3s (LL-HLS) — meets the "no strict latency" bar.
- Versatile: same HLS endpoint could be reused by any Cast/browser.
- **Risks**: `rust_cast` is old (2023) — must verify its media-load of HLS
  against current firmware. If broken → Option 2.

### Option 2 — Custom Web Receiver + HTTP streaming (MORE work, registration)
- Register a receiver app ID (Cast SDK console), build a web receiver
  (Shaka player for HLS, or WebRTC/MediaSoup for sub-second).
- Best latency + control; **requires Google developer registration + account**,
  larger build surface. Only if Option 1 is impossible.

### Option 3 — "Cast a web terminal page" (NO video encode)
- Serve an HTML page that renders the terminal (xterm.js + websocket from herdr
  pty), then either load it in a receiver or tab-cast it.
- No encode pipeline; but depends on xterm.js/live websocket in a receiver, and
  plain tab-cast quality/latency depend on the sender machine. Medium effort,
  different failure modes.

### (Deferred) Option 4 — Non-Chromecast hardware
- If Chromecast proves intractable, revisit: cheap HDMI stick (RPi) running a
  display client, or `pip-serve`/desktop-portal to RDP/VNC. **Out of scope** for
  the Chromecast-targeted solution but the most "versatile" escape hatch.

## Feasibility + versatility assessment
| Opt | Effort | Latency | No registration | Versatile | Feasibility risk |
|---|---|---|---|---|---|
| 1 | Low | 1-3s ✓ | ✓ | High (HLS reused anywhere) | rust_cast maturity |
| 2 | High | <1s | ✗ (register) | Med | Google policy |
| 3 | Med | ~1-3s tab | depends | Med | tab-cast quality, xterm+ws |
| 4 | Med | — | — | — | changes hardware |

**Recommendation to discuss**: Option 1 first (thin, no registration, reuses the
HLS stream everywhere, good feasibility if `rust_cast` validates), keep Option 2
as the escalation if the Default Media Receiver path is unsupported.

## Open items blocking a final decision
1. `rust_cast` smoke-test against a real (current) Chromecast: can it
   `media_load` an HLS URL onto CC1AD845? (De-risks Option 1.)
2. herdr/ptty details: spawn-a-pty vs attach (sets capture bridge in front of
   `alacritty_terminal`).
3. Chromecast model (codec ceiling) + whether Host→CC is wired or Wi-Fi.
4. Palette/typography target for readable 1080p terminal feed.
