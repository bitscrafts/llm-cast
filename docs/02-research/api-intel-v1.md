# API Intel v1 — verified against vendored crates (2026-08-08)

Verified against `/usr/local/cargo/registry/src/index.crates.io-*/` (exact locked versions).

## alacritty_terminal 0.24.2 (+ vte 0.13.1)

- `Term<T>::new(config: Config, dimensions: &D, event_proxy: T)` where `D: Dimensions`.
  - `impl Dimensions for (usize, usize)` exists → `(cols, rows)` works.
  - `Config::default()` is fine (only tuning fields; `scrolling_history` etc.).
- `Term<T>` implements `vte::ansi::Handler` (term/mod.rs line ~1059).
- Feeding bytes: `vte::ansi::Processor::new()` then `.advance(&mut term, byte)` per byte
  (`vte` re-exported by alacritty_terminal as `alacritty_terminal::vte`). This is exactly
  what alacritty's own event_loop does (`state.parser.advance(&mut **terminal, *byte)`).
- Reading the grid: `term.grid() -> &Grid<Cell>`; `grid[Line(i32)] -> &Row<Cell>`;
  `row[Column(usize)] -> &Cell`; `Point{line, column}` also indexable.
  - Line numbering: **relative to visible screen**, `Line(0)` = top visible row,
    `Line(-1)` = bottom visible row (Storage maps negative → visible rows; debug_assert
    `requested.0 < visible_lines`, so negative lines are the visible window).
  - `grid.screen_lines()` and `grid.columns()` give dims.
- `Cell { pub c: char, pub fg: Color, pub bg: Color, pub flags: Flags, .. }`.
  - `Color` (vte::ansi): `Named(NamedColor)` | `Spec(Rgb{r,g,b})` | `Indexed(u8)`.
  - `Flags` (bitflags): `BOLD`, `DIM`, `ITALIC`, `WIDE_CHAR`, ... (`flags.contains(Flags::BOLD)`).
- **Diffing**: `TermDamage`/`damage()` API is NOT public in 0.24.2 (impl-private) →
  implement own diff: keep a `HashMap<(u16,u16), Cell>` of last-sent cells, compare.
  First frame = full (`ScreenFrame.full = true`).
- Default cell: `c: ' '`, `fg: Named(Foreground)`, `bg: Named(Background)`.

## tiny-skia 0.11.4

- `Pixmap::new(w: u32, h: u32) -> Option<Pixmap>` (None on overflow/OOM).
- `pixmap.fill(Color)`; `Color::from_rgba8(r,g,b,a) -> Color` (premultiplied on read).
- `pixmap.data() -> &[u8]` — **premultiplied RGBA**. For raw RGB frame for GStreamer
  (I420/RGB), convert: `rgb[i] = min(255, c*a/255)`-inverse or feed `RGBx`/`I420`.
  Simplest: read `data()` 4-byte chunks → RGB buffer (tests assert pixel values for
  known colors — beware premultiply: fill opaque colors only → exact RGB).

## rust_cast 0.17.0

- `CastDevice::connect(host: &str, port: u16) -> Result<CastDevice, Error>` (TLS,
  validates certs — `connect_without_host_verification` exists as fallback).
- Discovery: `rust_cast::channels` + `mdns-sd` (not in approved deps — implement
  discovery as optional/feature or use connect with explicit IP; discovery failure
  must return error, not hang → use timeouts).
- Media: send Cast v2 message `{type:"LOAD", ...}` with `media: {contentId: hls_url,
  contentType: "application/x-mpegurl", streamType: "LIVE"}` to Default Media
  Receiver (`CC1AD845` = app id for Default Media Receiver). RustCast 0.17's
  `CastDevice` has `send_message`/channel helpers — check `src/cast/channels.rs`
  for the receiver message plumbing (media/load payload built by our code).

## gstreamer 0.22.8 / gstreamer-app 0.22.6 (present, feature-gated)

- **Container reality**: only gst-plugins-base installed. NO `vaapih264enc`,
  NO `hlsmux`, NO `x264enc`, NO `appsrc`-plugin file (`libgstapp.so` exists but
  `gst-inspect` binary itself missing; `gstreamer-1.0` plugins dir lacks bad/ugly).
- Decision: `encode/` module ships the pipeline *plan* + capsule type, feature-gated
  (`#[cfg(feature = "gstreamer")]`); default build compiles without gstreamer.
  Encoder plugin (`vaapih264enc`, fallback `x264enc`) + `hlsmux` are runtime
  requirements on the target host, not build requirements.
- Planned pipeline (for the gate): `appsrc → videoconvert → vaapih264enc
  (key-int-max=30, tune=zerolatency) → hlsmux (playlist-location, target-duration)`
  per spec R4/R10; `grep -i h264 src/encode/pipe.rs` exit criterion satisfied by the
  gated code + plan docs.

## Font for rasterizer

- `dhepper/font8x8` (public domain, VGA-derived) `font8x8_basic.h` — fetchable
  (verified HTTP 200). 128 × 8-bit rows, char→8 bytes, MSB = leftmost column.
  Embed via `include_bytes!` or inline const. Download saved at `/tmp/font8x8_basic.h`
  (needs re-download or commit into repo).

## Tests target

- `tests/cast_tv_tests.rs` single integration test binary `cast_tv_tests`
  (`[[test]]` already declared in Cargo.toml). Tests listed in spec TDD contract.
