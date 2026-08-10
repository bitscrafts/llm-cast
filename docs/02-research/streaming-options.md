# Streaming options + encoding (research)

Status: DRAFT (2026-08-07). Primary source: Google "Supported Media for Google
Cast" (`developers.google.com/cast/docs/media`, loaded 200) + "Web Receiver
Overview".

## How content reaches a Cast device (authoritative)
A Cast **receiver app is an HTML5/JS web app** running on the device, in a
Chrome-like environment. It fetches media over the network and renders it. Key
facts from Google docs:
- Receiver apps support a variety of **media formats, containers, codecs, and
  delivery methods**; some need the Web Receiver SDK.
- **CORS is mandatory** — a stream server must send proper CORS headers or the
  receiver cannot play the stream ("If you're having problems playing streams on
  a Cast device, it may be an issue with CORS").
- Adaptive streaming (HLS/DASH) with DRM is supported, often requiring the Web
  Receiver SDK.
- There is a **"HLS on Shaka Player"** section (Custom Web Receiver) — HLS is a
  first-class path for a custom receiver.

### Two architectures driven by this
1. **Encode terminal → video stream (HLS) on the host**, and a **custom Web
   Receiver app** (Shaka player) plays the HLS stream. Requires: an ffmpeg
   encoder + an HLS/segment server + CORS + a Cast receiver app (needs Google
   **registration** for a custom receiver).
2. **Serve a web terminal page** (no video) and cast that **page/tab** or load
   it inside a receiver webview. Simpler codecs, but tab-cast quality depends on
   the sender machine's encode power + latency.

## Video codecs per device (Google's own table)
Universal safe bet: **H.264 High Profile** (all generations).
| Device | H.264 | also supported | up to |
|---|---|---|---|
| Chromecast (3rd gen) | HP L4.1 | VP8 | 1080p/30 |
| Chromecast 3rd Gen / Ultra | HP L4.2 | VP8 | 1080p/60 |
| Chromecast Ultra | HP L4.2 | HEVC/H.265, VP9, HDR | 4K/60 |
| Chromecast w/ Google TV | HP L5.1 | HEVC, VP9-2, HDR | 4Kx2K/30/60 |
| Google TV Streamer | HP L5.2 | HEVC, VP9-2, **AV1** | 4Kx2K/60 |

Takeaways:
- **H.264** = universal. If we want 1080p on any modern device, H.264 HP L4.2+ is
  the floor; 4K needs newer devices + HEVC/AV1.
- For a terminal feed, **720p/1080p H.264 is more than enough and safest**.
- Hardware encode (VA-API on Intel/AMD GPUs, NVENC on NVIDIA) makes per-node
  H.264 cheap; a terminal screen is low-motion so bitrate needs are tiny.

## Audio codecs (if the terminal session has audio/shell audio)
Opus, AAC (HE/LC), MP3, FLAC, Vorbis, WAV/LPCM; AC-3/E-AC-3 pass-through.
**Opus is the low-latency pick.** Often terminal->TV needs no audio at all.

## Latency / interactivity reality
- HLS adds segment latency (typically ~2-3 segments of buffering; ~3-10s with
  standard 4-6s segments). Plain HLS is **NOT** great for interactive terminal.
- Lower-latency paths to investigate next:
  - Shaka on **LL-HLS / Low-Latency HLS (partial segments, 1-2s)**
  - WebRTC (sub-second) rendered in a receiver webview — WebRTC in a custom
    Cast receiver app is viable but more build work.
  - **MJPEG snapshot stream** (media receiver plays a slideshow of JPEGs) —
    very low latency, low CPU, but not fluid text rendering.
- "terminal content" is mostly static text between changes; a modest framerate
  (2-10 fps) is plenty, which keeps H.264 bitrate tiny even for LL-HLS.

## To verify next
- Whether the "outer/herd" muxer exposes: an HTTP/web view, a tmux-ish control
  socket, or only an SSH/pty attach. (Determines if we render text->HTML (cheap)
  or text->video (encode pipeline).)
- Chromecast exact model/gen (from codec table above) + LAN topology + whether
  host and TV on same Wi-Fi.
- Cast **registration** requirement: a custom receiver currently needs an app ID
  registered via Google Cast SDK console / DIAL. Confirm current policy.
