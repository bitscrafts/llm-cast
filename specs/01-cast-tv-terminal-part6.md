# Spec: cast-tv-terminal — Part 6/6

**Parent-Spec**: `01-cast-tv-terminal.md`
**Part**: 6 of 6
**Covers**: the FULL pipeline integration — wire R1 (real capture source:
tmux/`herdr` `pipe-pane` output file → `PipeSource`), R2/R3 (already shipped,
now composed), R4 (real H.264→HLS encoder behind the `gstreamer` feature,
amending part 4's `hlsmux` sketch which is not a real element), R5 (HLS
serving becomes **store-driven live** output, amending part 3's static
stand-ins), R6 (cast wiring that reuses the milestone-1-verified session),
R11 (safe degrade — cast failure is non-fatal, never hangs), plus a new
`pipeline` coordinator module and the `mirror` operator binary.
**Status**: SPECIFIED — WRITTEN 2026-08-16 (after the milestone-1 smoke test
PASS on `10.10.10.208`). Not yet implemented.

## Overview

Parts 1-5 shipped every stage of the cast path as seams, and the milestone-1
smoke test proved the cast leg for real (`castctl` loaded an HLS URL and Big
Buck Bunny played on the device). What is still standing in is: the capture
source is a trait with no production reader, the encoder is a compile-only
GStreamer sketch (`hlsmux` — not an element name), and the HTTP server serves
a fixed placeholder playlist/segment instead of encoder output. This part
replaces each stand-in with the real thing and adds a long-running
coordinator + operator binary that runs the whole pipeline: read the pane →
vte grid → rasterize → encode → serve → cast, until the operator stops it.

Verifiability contract (proven by the milestone-1 arc): everything that can
be validated in this container (no gstreamer system packages, no tmux, no
route back to the device) is validated by default-feature tests over trait
seams. The real GStreamer encode and the live device load are **operator
steps** run on a LAN-reachable host, exactly as `castctl` was. The in-container
exit criteria do not require gstreamer, tmux, or the device.

Milestone-2 target (operator): `mirror --source <pipe-pane file> ... --device
10.10.10.208` shows the **live** herdr/tmux pane on the TV.

## Modules in this part

```
src/
├── capture/
│   ├── mod.rs         MODIFY: `pub mod pipe;` + `pub use pipe::PipeSource;`
│   └── pipe.rs        (NEW)  PipeSource: reads a tmux pipe-pane output FILE (ByteSource impl)
├── encode/
│   └── pipe.rs        MODIFY: add `Encode` trait + `EncodeError` + `NullEncoder`
│                        (unconditional); rework the gstreamer path into a real
│                        `GstEncoder` + `build_pipeline` (x264enc/vaapih264enc → hlssink2).
│                        Remove the `hlsmux` sketch and the `H264_ENCODER` const.
├── serve/
│   ├── mod.rs         MODIFY: `pub mod store;` + `pub use store::{DirStore, MapStore, MediaStore};`
│   ├── store.rs       (NEW)  MediaStore trait + MapStore (tests/dry-run) + DirStore (encoder output dir)
│   └── server.rs      MODIFY: `app(store: Arc<dyn MediaStore>)` reads live from the store;
│                        remove the static PLAYLIST/SEGMENT_BYTES stand-ins; add
│                        `pub async fn serve_hls(store, listener)`.
├── pipeline/
│   ├── mod.rs         (NEW)  `pub mod coordinator;` `pub use coordinator::{Pipeline, PipelineConfig};`
│   └── coordinator.rs (NEW)  the loop: source → emu → rasterize → encode, throttled + keepalive
└── bin/
    └── mirror.rs      (NEW)  operator binary: `mirror --source <file> [--bind A:P] [--size WxH]
                             [--outdir DIR] [--encoder x264|vaapi] [--device IP] [--url-base URL] [--no-cast]`
```

## Key Data Structures (owned/amended by this part)

```rust
/// One frame sink — a rasterized RGBA canvas is submitted, an HLS stream
/// comes out the other side. Default features ship `NullEncoder`; the real
/// GStreamer encoder is `#[cfg(feature = "gstreamer")]`.
pub trait Encode {
    /// Encode one RGBA canvas (`width*height*4` bytes, row-major).
    fn submit_frame(&mut self, rgba: &[u8], width: usize, height: usize) -> Result<(), EncodeError>;
    /// The URL the HLS stream is served at — what the cast LOAD targets.
    fn stream_url(&self) -> String;
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError { /* e.g. Gst(String), Buffer(String) */ }

/// Default-features encoder: records submissions, emits nothing. In-container
/// tests + dry-run validation prove the wiring minus the codec.
pub struct NullEncoder { /* submitted: usize, last_dims, url: String */ }
impl NullEncoder { pub fn new(url: String) -> Self; pub fn submitted(&self) -> usize; }

/// Real H.264 → HLS encoder (see Implementation Notes for the pipeline).
#[cfg(feature = "gstreamer")]
pub struct GstEncoder { /* pipeline, appsrc, url */ }
#[cfg(feature = "gstreamer")]
impl Encode for GstEncoder { /* ... */ }

/// What the HLS HTTP server reads from — the live artifact store.
pub trait MediaStore: Send + Sync {
    fn playlist(&self) -> Option<String>;               // raw live.m3u8 text
    fn segment(&self, name: &str) -> Option<Vec<u8>>;   // one media segment
}
pub struct MapStore { /* in-memory, seedable — tests + default-features dry-run */ }
pub struct DirStore { dir: PathBuf }                    // reads the hlssink2 output dir

/// One throttled pipeline step; injectable `now_ms` makes the cadence testable.
pub struct PipelineConfig {
    pub keepalive_ms: u64,  // push a frame even when idle, to keep HLS segments flowing (default 1000)
    pub tick_ms: u64,       // loop granularity (default 10)
}
pub struct Pipeline<S: crate::capture::ByteSource, E: Encode> {
    bridge: crate::capture::Bridge<S>,
    encoder: E,
    config: PipelineConfig,
    buf: Vec<u8>,           // reusable RGBA canvas (sized from the emu grid × 8)
    last_submit_ms: u64,
}
impl<S: ByteSource, E: Encode> Pipeline<S, E> {
    pub fn new(bridge: Bridge<S>, encoder: E, config: PipelineConfig) -> Self;
    /// One step: poll the source, submit on damage, keepalive when idle.
    pub fn poll_and_submit(&mut self, now_ms: u64) -> Result<(), EncodeError>;
    /// Loop `poll_and_submit` with real time until a shutdown signal.
    pub fn run(&mut self);
}
```

`serve::server::app` keeps its name but the signature becomes
`pub fn app(store: Arc<dyn MediaStore>) -> Router`. `serve::server::serve_hls`
is `pub async fn serve_hls(store: Arc<dyn MediaStore>, listener: tokio::net::TcpListener)`
(runs `axum::serve`; the caller provides the already-bound listener so tests
can bind `127.0.0.1:0`).

## TDD Contract

| Test Name | Given | Expects |
|-----------|-------|---------|
| `test_pipe_source_reads_available_bytes` | temp file containing `b"hi\x1b[31m"`; `PipeSource::open` on it | `read_available(&mut buf)` returns 7 (2 + 5 = the file's length) and copies the bytes; a second read returns 0 (at EOF) |
| `test_null_encoder_counts_frames` | `NullEncoder::new("http://h:8080/live.m3u8")` | 3× `submit_frame(&[0u8;4], 8, 8)` → `submitted()==3`; `stream_url()==` the URL |
| `test_pipeline_submits_changed_frames` | Pipeline over an in-memory source emitting an ANSI string, NullEncoder; `poll_and_submit(0)` | `submitted() >= 1`; the encoder saw the emu grid size × 8 canvas |
| `test_pipeline_skips_unchanged_frames` | after the first submit, a second `poll_and_submit(now)` with no new bytes | `submitted()` unchanged (diff frame is empty → no encode) |
| `test_pipeline_keepalive_after_idle` | unchanged screen; `poll_and_submit(now = keepalive_ms + 1)` | exactly one more submission (the keepalive frame) |
| `test_served_playlist_reads_from_store` (AMENDS part-3's `test_hls_playlist_has_cors` + `test_served_segment_bytes`) | `app(Arc::new(MapStore::seeded(...)))` with a playlist + one segment | GET `/live.m3u8` → 200 + the seeded playlist text; GET `/segment/seg0.ts` → 200 + bytes; unknown → 404; `Access-Control-Allow-Origin` header present on both 200s |
| `test_dir_store_reads_output_dir` | temp dir containing `live.m3u8` + `seg_00000.ts` | `playlist()` == file text; `segment("seg_00000.ts")` == file bytes; unknown name → `None` |
| `test_serve_hls_binds_and_responds` | `serve_hls(seeded MapStore, listener bound on 127.0.0.1:0)` spawned as a task | GET `/live.m3u8` over the returned address → HTTP 200 |

All 23 existing tests stay green (the two part-3 HLS tests are reworked into
`test_served_playlist_reads_from_store`; their 200 + CORS assertions are
preserved). The 10-test parent TDD contract remains fully present.

## Implementation Notes

- **Capture — `PipeSource` (R1, real)**: `PipeSource { file: File }` with
  `PipeSource::open(path: impl AsRef<Path>) -> Result<Self, BridgeError>`
  opening the path for read. Reads are plain `File::read` into the caller's
  buffer — a regular file read never blocks and returns 0 at the current EOF,
  so no `O_NONBLOCK`/`libc` dependency is needed. tmux/`herdr` integration is
  the operator's `tmux pipe-pane -t <target> <file>` writing raw escape bytes
  into that file (tmux-compatible, same mechanism herdr exposes). Because the
  file grows, the `File` cursor advances across polls — exactly the `ByteSource`
  contract part 2 pinned (`read_available` copies what is available, never
  blocks). The existing `test_capture_bridge_feeds_bytes_to_vte` keeps passing
  (its in-memory fake stays in the test file).
- **Encode (R4, real) — reworks part 4's `encode/pipe.rs`**: part 4 shipped
  `parse_launch("appsrc ... vaapih264enc ! hlsmux ...")` as a compile-only
  sketch. `hlsmux` is **not a GStreamer element name** (the HLS muxers are
  `hlssink`/`hlssink2` from gst-plugins-bad) — it would fail at runtime. This
  part replaces it with the real, correct pipeline:
  ```
  appsrc name=src format=time is-live=true do-timestamp=true \
    caps="video/x-raw,format=RGBA,width=W,height=H,framerate=F/1" \
    ! videoconvert \
    ! {x264enc tune=zerolatency speed-preset=veryfast bitrate=800 key-int-max=30
       | vaapih264enc} \
    ! hlssink2 location=OUTDIR/segment/seg_%05d.ts \
               playlist-location=OUTDIR/live.m3u8 \
               target-duration=1 max-files=30 playlist-root=ROOT
  ```
  - `--encoder x264` (default, portable, software) vs `--encoder vaapi`
    (`vaapih264enc`, satisfies parent R10 "hardware H.264 via VA-API" on a
    VA-API-capable host). The chosen element is substituted into the pipeline
    string at build time.
  - `hlssink2` emits ~1 s segments into `OUTDIR/segment/` and writes
    `OUTDIR/live.m3u8` (live, `playlist-length=0` → no ENDLIST; `max-files=30`
    → rolling ~30 s window).
  - `playlist-root=ROOT` makes the playlist list **absolute** segment URLs at
    `ROOT/seg_%05d.ts`; ROOT is computed by `mirror` from `--url-base` by
    replacing `/live.m3u8` with `/segment` (so the DMR fetches
    `http://host:8080/segment/seg_00000.ts` → our `/segment/:name` route). The
    operator must pass a `--url-base` the **device** can reach (LAN IP, not
    `0.0.0.0`); verify on the host with `curl -s <url-base>` before casting.
  - `GstEncoder` owns the built `Pipeline` + `appsrc`; `submit_frame` pushes an
    `appsrc` buffer with a running timestamp; `stream_url()` returns the URL
    passed at construction. Every `gstreamer::prelude` call that can fail is
    `map_err`'d into `EncodeError::Gst` — no panics.
  - Runtime packages the LAN host needs: `libgstreamer1.0-dev`,
    `libgstreamer-plugins-base1.0-dev`, `gstreamer1.0-plugins-{base,good,bad,ugly}`,
    `x264` (build also needs those `-dev` packages for the `gstreamer` rust crates).
    **Not** an in-container exit criterion — gstreamer is absent here.
  - `H264_ENCODER` const is removed (superseded by `--encoder`).
- **Serve (R5, live) — amends part 3's static stand-ins**: `server.rs` drops
  the `PLAYLIST`/`SEGMENT_BYTES` consts and `segment_bytes()`; `app()` takes
  `Arc<dyn MediaStore>` and the two handlers read `store.playlist()` /
  `store.segment(name)`, 404 when `None`. The CORS layer is unchanged
  (`AllowOrigin::any()`). `serve_hls(store, listener)` runs
  `axum::serve(listener, app(store))`. `DirStore` is the production store
  (reads the hlssink2 output dir); `MapStore` serves tests and the
  default-features dry-run. `map` is `std::collections::HashMap<String, Vec<u8>>`
  wrapped in `Mutex` (or `RwLock`) — no new deps.
- **Coordinator (new `pipeline` module)**: `poll_and_submit(now_ms)` =
  `bridge.poll()` → if the emu emitted a **changed** diff frame
  (`!frame.cells.is_empty()`; `ScreenFrame` may need `Clone`/`PartialEq`
  derives added — derive-only, no behavior change) reallocate/reuse `buf` sized
  `width*8 × height*8 × 4`, `rasterize(&frame, &mut buf)`, `encoder.submit_frame`.
  If unchanged but `now_ms - last_submit_ms >= keepalive_ms`, submit the current
  canvas anyway (a static screen must still produce segments for HLS to keep
  flowing). `run()` loops with real time (`tick_ms` sleep) until a shutdown
  signal (`tokio::signal` — tokio is already "full"). The coordinator is **sync**
  and generic over `S: ByteSource, E: Encode` — fully testable in-container.
- **`mirror` binary (operator)**: manual arg parse (no clap — same style as
  `castctl`), `--help` prints usage and exits 0. Flow: open `PipeSource` →
  `Emulator::with_size(w, h)` (default `160x45` → 1280×360 px canvas) →
  `Bridge` → `Pipeline` with `NullEncoder` (default features) or `GstEncoder`
  (`--features gstreamer`, `--encoder x264|vaapi`, `--outdir`). Store =
  `DirStore::new(outdir)` under the gstreamer feature, else a `MapStore`
  seeded with a placeholder playlist+segment so the served URL returns 200 for
  dry-run validation. Bind `TcpListener` on `--bind` (default `127.0.0.1:8080`)
  and `serve_hls` in a tokio task. Then the cast leg:
  - Requires `--device` AND the `cast` feature; under default features,
    `--device` prints the same "built without the cast feature — no session
    will be sent; rebuild with --features cast" notice as `castctl` and
    **continues serving** (R11).
  - Builds `Sender::new(Box::new(move || Ok(DeviceAddr::new(ip))))` and calls
    `send_load(&url)` **once** after the server is up. URL = `--url-base` if
    given, else `http://<bind-host>:<port>/live.m3u8` (if bind-host is a
    wildcard, a `--url-base` is required for cast — the device cannot resolve
    `0.0.0.0`).
  - Cast failure (device unreachable, session error) is logged and **non-fatal**:
    `mirror` keeps serving; it never hangs and never exits on cast error (R11).
  - Exit codes: `0` clean run / `--help`; `2` usage error; non-zero on fatal
    startup errors (bad `--source`, bind failure). No `.unwrap()`/`.expect()`
    outside `fn main`'s arg-parsing checks.
- **CROSS-REFERENCE**: amends part 3 (serve store-driven; its two HLS tests
  reworked, CORS assertions preserved) and part 4 (`encode/pipe.rs` reworked
  to the real pipeline; `H264_ENCODER` removed). Part 5's cast seam and
  milestone-1's transport-connect fix are untouched and reused as-is.

### Operator steps (NOT in-container exit criteria)

```bash
# 1. In-container validation of everything minus codec+device:
cargo build && ./target/debug/mirror --source /dev/null --size 80x24 \
    --bind 127.0.0.1:8080 --no-cast &   # then: curl -s 127.0.0.1:8080/live.m3u8 → 200

# 2. Milestone-2 device test (LAN-reachable host with gstreamer + plugins):
cargo build --release --features cast,gstreamer
tmux pipe-pane -t work:0.0 /tmp/cast-hls/pane.out
./target/release/mirror --source /tmp/cast-hls/pane.out --size 160x45 \
    --bind 0.0.0.0:8080 --outdir /tmp/cast-hls \
    --url-base http://<LAN-IP>:8080/live.m3u8 --device 10.10.10.208
# Definitive check: the TV shows the LIVE herdr/tmux pane.
```

## Exit Criteria

- [ ] `cargo test --test cast_tv_tests 2>&1 | grep -q "test result: ok"`
- [ ] `cargo check --features cast 2>&1 | grep -q "Finished"`
- [ ] `test -f src/pipeline/coordinator.rs && test -f src/capture/pipe.rs && test -f src/serve/store.rs && test -f src/bin/mirror.rs`
- [ ] `grep -q "pub trait Encode" src/encode/pipe.rs && grep -q "pub trait MediaStore" src/serve/store.rs`
- [ ] `grep -q "hlssink2" src/encode/pipe.rs && grep -q "PipeSource" src/capture/pipe.rs`
- [ ] `grep -q "pub fn app" src/serve/server.rs && grep -q "pub fn serve_hls" src/serve/server.rs`
- [ ] `! grep -rE '\.unwrap\(\)|\.expect\(' src/pipeline src/capture/pipe.rs src/serve src/bin 2>/dev/null | grep -v '//' | grep -v '#\[cfg\(test\)\]' | grep -v test`
- [ ] `cargo build 2>&1 | grep -q "Finished" && timeout 10 ./target/debug/mirror --help >/dev/null 2>&1`

## Guardrails

- Do not run `cargo`, `rustc`, `clippy` outside the TDD cycle steps
- Do not add public API surface not specified in Requirements
- Do not use `.unwrap()`, `.expect()`, or `panic!()` in production code paths
- Do not modify files outside the project root
- Do not add dependencies to `Cargo.toml` — everything here is within the
  already-approved set (`tokio` full already includes `signal`; `std::fs` for
  the pipe/store). No `clap`, no `libc`, no `tempfile`, no HTTP client.
- **Approved dependencies**: `alacritty_terminal`, `rust_cast` (optional),
  `gstreamer*` (optional), `tiny-skia`, `tokio`, `axum`, `tower-http`,
  `serde`, `serde_json`, `thiserror`
- **Verifiability**: in-container exit criteria must not require gstreamer
  system packages, tmux, or the device — those are operator steps above.
- **PIVOT GUARDRAIL (unchanged)**: do NOT build a custom Cast receiver,
  register with Google Cast, or use WebRTC for this milestone. rust_cast
  `media_load` of HLS is proven; `mirror` reuses that session. If the live
  pipeline cannot play HLS on the device, STOP and report for an explicit
  Option-2 decision.

On any ambiguity, stop and report back, do not guess.
