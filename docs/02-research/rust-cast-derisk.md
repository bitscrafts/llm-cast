# rust_cast de-risking (research)

Status: DRAFT (2026-08-07). Sources: crates.io (live), GitHub API (forks),
rust-cast README (earlier).

## Current facts
- Crate `chromecast` = `rust_cast` (GitHub `tsirysndr/rust-cast`), a Rust **Cast v2
  sender** (derived from Chromium Open Screen).
- **crates.io**: v0.18.2, **2023-03-30** last update, **~8.9K downloads**
  (small audience). Low maintenance signal.
- **GitHub forks: 0** (API returned empty) — no maintained community fork to fall
  back on if the main branch is stale/broken.

## De-risking assessment
We want this crate to send a Cast v2 **media-load** (HLS URL) onto the built-in
**Default Media Receiver (CC1AD845)**. The vectors of failure:
1. **Discovery**: mDNS/DNS-SD `_googlecast._tcp` discovery must still work on the
   Chromecast; old libs occasionally drop TLS/DISCOVER semantics. Medium risk.
2. **Cast v2 protocol / TLS**: Chromecasts migrated device cert/TLS handling;
   a 2023 sender against 2026 firmware could hit handshake/`cast.channel`
   mismatches. Medium risk.
3. **media-load payload**: pushing `media/load` with an `HlsSegmentFormat` /
   HLS content type to the Default Media Receiver is the intended path, but it
   must be verified on a real device. This is the single highest-value smoke test.

## Findings / options
- **No maintained fork** → options are: (a) use 0.18.2 as-is and smoke-test; (b)
  patch it lightly (it's one repo) if a specific message is stale; or (c) if the
  sender path is fundamentally broken, fall back to Option 2 (custom Web Receiver
  + Shaka/WebRTC) which needs Google Cast registration — more work, gated by
  Google policy.
- Because the payload we control (our own HLS server on the host) is standard,
  the most likely successful path is media-load of an HLS URL. The unknown is the
  sender's protocol layer, not the media format.

## Recommended de-risk action (next)
- Stand up a minimal **smoke-test harness**: `rust_cast` discover + connect +
  `media_load` an HLS URL (from a throwaway ffmpeg HLS on the host) → observe on
  the TV. This is the pivotal gate for the whole architecture (Option 1).
- In parallel, prototype the **capture bridge** (see capture-bridge.md) so the
  encode side is ready regardless of the sender verdict.

## Memory/doc trail
- Insight: `chromecast-tv-mirror/research/rust-cast-derisk` (this note).
