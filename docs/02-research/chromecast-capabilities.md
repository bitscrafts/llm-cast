# Chromecast capabilities (research)

Status: DRAFT (2026-08-07). Sources: Wikipedia "Google Cast" article (loaded;
Google's own `/docs` 404'd several paths — will re-verify against
`developers.google.com/cast/docs/media/supported-media`).

## What a Chromecast is (and is NOT)
- A **headless Google Cast receiver**: a small Chrome-browser-like environment
  the device boots into. You cannot install or run arbitrary software on it.
- No local display OS, no user shell, no filesystem you control.
- The host controls it by *casting*: a **sender app** discovers the device on the
  LAN, opens a secure channel, and loads/controls a **receiver app**.

## The two official ways content reaches a Cast device
1. **Cast-enabled apps** (sender + receiver pair). The sender sets/controls
   playback; the **receiver app is a web app** running in the device's Chrome-like
   environment and pulls the media (usually directly over the network). This is
   how YouTube/Netflix/Plex etc. work.
2. **Mirroring**:
   - **Chrome tab casting** from a laptop (Cast extension). The tab is encoded
     on the *sender* (CPU cost on the sender) and pushed. Quality depends on the
     sending machine's processing power.
   - **Android screen mirroring** (degraded quality).

## Receiver-app streaming protocols (custom receivers)
A custom receiver app can play, beyond a plain HTML `<video>`:
- MPEG-DASH
- **HTTP Live Streaming (HLS)**
- Microsoft Smooth Streaming

This is key: a custom Cast receiver can render an **HLS/MSE live stream** — a
plausible path for terminal→video.

## Supported codecs (from the article; device-gen dependent)
Video:
- H.264 High Profile up to Level 4.1/4.2/5.1 (varies by device generation) —
  commonly 1080p@60, some 4K
- HEVC/H.265 Main + Main10 up to L5.1 (up to 4K@60 on capable devices)
- VP9 Profile 0/2 up to L5.1 (up to 4K@60)

Practical takeaway: **H.264 is the safest, most universally supported** codec to
feed a Cast device. HEVC/VP9 are gen-dependent.

Audio:
- Opus, AAC (LC/HE), MP3, Vorbis, FLAC, WAV/LPCM; AC-3/E-AC-3 pass-through on
  capable devices. Opus is a strong low-latency choice.

Images: BMP, GIF, JPEG, PNG, WEBP (gen-1 limited to 720p).

## Red flags / to verify
- I still need to confirm: exact encoding used for **tab-cast (what container/
  codec Chrome pushes)**, to see if we could reuse it.
- Need Chromecast **model/generation** and its Wi-Fi generation for the latency
  budget.
- Wikipedia is secondary; primary sources below to pull next:
  - `developers.google.com/cast/docs/media/supported-media`
  - `developers.google.com/cast/docs/web_receiver`
  - Chromecast specs (google.com/chromecast) for codecs/latency

## Direction this hints at
- Since we cannot install a display client on the Chromecast, the viable routes
  are (a) **encode the terminal to a video stream on the host** and render it
  via a receiver app (HLS/WebRTC), or (b) **serve a web page showing the terminal
  session and cast that page/tab**. Docs stored under `02-research/`.
