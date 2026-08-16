# Milestone-1 — Device Smoke Test (PASS)

**Date**: 2026-08-16
**Status**: ✅ PASS — operator-confirmed video playback on a real Chromecast
**Project phase**: spec-01 complete (5 parts) → milestone-1 verified → next:
full pipeline integration (spec-01 part6)

---

## What milestone-1 proved

**rust_cast CAN `media_load` an HLS stream onto a real Chromecast.** The
pivot guardrail was NOT triggered: no custom receiver, no Cast SDK
registration, no WebRTC fallback (Option 2 stays out of scope).

> `castctl 10.10.10.208 https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8`
> → `castctl: PASS — media load sent` (exit 0) → **Big Buck Bunny played on
> the TV** (operator-confirmed, DMR `CC1AD845`).

## What was built

| artifact | role |
|---|---|
| `src/cast/sender.rs` | `DeviceAddr {host, port}` (default 8009), discovery closure, `send_load` |
| `src/cast/session.rs` (`--features cast`) | real rust_cast 0.17 session: connect → transport connect → heartbeat → launch_app → media.load |
| `src/bin/castctl.rs` | operator CLI: `castctl <device-ip> <hls-url>`; non-zero exit on failure; no unwrap/expect/panic |
| tests | 23/23 default, 22/22 with `--features cast` (one network-gated test excluded) |

Gate GREEN: fmt / check / clippy -D warnings / test — all PASS.

## Two findings that mattered

1. **The canonical BBB URL is dead.** `commondatastorage.googleapis.com`
   returns 403 (anonymous access locked down, 2026). Verified via curl. The
   working stream is `https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8`
   (HTTP 200, no redirect, 240p–1080p). Apple's bipbop URL 302-redirects;
   akamai sintel is also 403 now. **Lesson: public test URLs rot — verify
   reachability before relying on them (premise check / exit criterion).**

2. **A real protocol bug caused the hang.** `media.load` without a connected
   app transport is dropped by the DMR, and rust_cast's `receive_find_map`
   then waits forever for a STATUS that never arrives (infinite hang; stopped
   by the 124 timeout). Canonical flow (verified against upstream rust_caster):
   `launch_app → connection.connect(&application.transport_id) → media.load`.
   Fixed in `55c9a05` (step 2b in `src/cast/session.rs`).

Also fixed: `test_sender_accepts_device_address` was gated to
`#[cfg(not(feature = "cast"))]` — under the cast feature its injected fake
address triggered a REAL connect to 192.168.1.50:8009 (~130 s hang). Unit
tests must not reach the network; the live path belongs to the device smoke
test.

## Working HLS test URLs (as of 2026-08-16)

- ✅ `https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8` — 200, no redirect
- ❌ `commondatastorage.googleapis.com/.../BigBuckBunny.m3u8` — 403
- ⚠️ Apple `devstreaming-cdn.apple.com/.../img_bipbop...m3u8` — 302 redirect
- ❌ `bitdash-a.akamaihd.net/content/sintel/hls/playlist.m3u8` — 403

## Memory keys stored (topic `chromecast-tv-mirror`)

- `implementation/rustcast-transport-connect` (0.9)
- `implementation/milestone1-smoke-pass` (0.9)

---

## Process lessons for pi-orchestration (the experiment's yield)

Milestone-1 was also a live test of the spec-driven loop. What held up and
what it suggests:

1. **The parts-split is the right shape for a small/local worker.** spec-01
   ran as five narrow parts (terminal core → capture/cast → serve/encode →
   closing sweep → real session), each with pinned modules, self-contained
   exit criteria and a seam-level acceptance test. Every part landed GREEN.
   This is the shape to replicate when the worker becomes laguna-xs-2.1
   (local, ~2B): the spec must carry 100% of the judgement.

2. **Unit tests must be network-free — even feature-gated ones.** The
   `--features cast` test reached a real socket (130 s hang). Suggested
   guardrail for spec-author/DIRECTIVES: *"feature-gated tests that would
   perform real network I/O must be compiled out of default-features runs;
   live paths are verified by operator tests, not unit tests."*

3. **External URLs rot.** The 403 on the canonical BBB URL cost a hang and a
   re-run. Suggested: verify reachability as part of Verified Premises
   (spec-author section) or as an explicit exit criterion (`curl -sf <url>`).

4. **The phase timeout did its job.** The hang was stopped by the 124
   timeout — the wall-clock bound that pi-orchestration learned the hard way
   is the one that actually stops runaways. Validated in the wild.

5. **Spec code sketches are guidance, not contract.** Two sketches in the
   part-5 spec could not compile as written (`&"CC1AD845".into()` — rust_cast
   has only `FromStr`; the `move || Ok(DeviceAddr::new(ip))` closure — E0507).
   The workhorse fixed both and reported the deltas. Suggested: mark code
   sketches as illustrative, and pin the API SHAPE (signatures) in the TDD
   Contract instead of literal snippets.

6. **Operator verification is a real phase.** The seam that mattered was the
   device, which no unit test can reach. The bundle's phases 0–8 end at
   commit; a "milestone verification" step (operator-run, post spec-batch) is
   worth documenting in PROCESS.md.

## Next steps

- spec-01 part6: full pipeline integration — live capture, real gstreamer
  encode on a LAN-reachable host, store-driven serve, `mirror` bin.
- Then: pidag (53 specs) as the final implementer.
