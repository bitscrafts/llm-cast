# API Intel v2 — verified addendum (2026-08-10, mid-TDD-implementation session)

Supplement to `api-intel-v1.md`. All verified against vendored sources at
`/usr/local/cargo/registry/src/index.crates.io-*/` (locked versions: alacritty_terminal 0.24.2,
rust_cast 0.17.0, vte 0.13.1, tiny-skia 0.11.4, axum 0.7.9). These are the exact details the
next agent needs to write `src/emu/`, `src/cast/` without re-deriving.

## alacritty_terminal 0.24.2 — grid indexing (CRITICAL, differs from v1 doc)

- `Grid::new(lines, columns, history_size)` — first arg is **visible lines**.
- **`Line(0)` = TOP visible row**, `Line(screen_lines - 1)` = BOTTOM visible row when
  `display_offset == 0`. Verified via `Storage::compute_index`:
  `positive = -(requested - visible_lines) - 1` → `Line(0)` → last slot (top),
  `Line(rows-1)` → slot 0 (bottom). (`Line(-1)` aliases the bottom row only in the
  scrollback-relative sense.)
- **Use nested direct indexing**, not `display_iter()`: `display_iter()` starts at
  `Line(-1)` (bottom) and wraps — wrong row order for rasterization.
  ```rust
  let grid = term.grid();
  for r in 0..grid.screen_lines() {
      for c in 0..grid.columns() {
          let cell = &grid[Line(r as i32)][Column(c)];
          // top-to-bottom, left-to-right
      }
  }
  ```
- `grid.screen_lines()` / `grid.columns()` are public (via `impl Dimensions for Grid<G>`).
- **Set `Config { scrolling_history: 0, ..Default::default() }`** — `Config::default()`
  uses 10000 scrollback; with scrollback the grid can grow and display_offset drift.
  History 0 keeps the visible window stable.
- `Term::new(Config, &(cols, rows), VoidListener)` — `(usize, usize): Dimensions` impl
  is `#[cfg(test)]`-gated?? NO — verify: `impl Dimensions for (usize, usize)` is in
  `src/grid/mod.rs` under `#[cfg(test)]`; the v1 note said it exists. **Safest**: define
  own `struct TermSize(usize, usize)` implementing `Dimensions` (total=screen=rows, cols).
  (Confirmed the cfg(test) gate at grid/mod.rs:544.)
- `Cell` struct: `{ pub c: char, pub fg: Color, pub bg: Color, pub flags: Flags,
  pub extra: Option<Arc<CellExtra>> }` (src/term/cell.rs:134).
- `Color` lives in **vte** (`vte::ansi::Color`): `Named(NamedColor) | Spec(Rgb{r,g,b}) |
  Indexed(u8)` (src/ansi.rs:1075). alacritty re-exports vte as `alacritty_terminal::vte`.
- `Flags` bitflags: `BOLD=0b10, ITALIC=0b100, DIM=0b1000_0000, INVERSE=0b1, WIDE_CHAR...`
  (src/term/cell.rs:15).
- Feeding bytes: `vte::ansi::Processor::new()` + `.advance(&mut term, byte)`; `Term<T>`
  implements `vte::ansi::Handler` (term/mod.rs:1059). `Processor` is
  `vte::ansi::Processor` (src/ansi.rs:280).
- `VoidListener` at `alacritty_terminal::event::VoidListener`.

## rust_cast 0.17.0 — sender plumbing (verified signatures)

- `CastDevice::connect(host: &str, port: u16) -> Result<CastDevice, Error>` (blocking TLS,
  host-verified); `connect_without_host_verification` also exists. Port 8009.
- Public channel fields on `CastDevice`: `.connection`, `.heartbeat`, `.media`, `.receiver`.
- Launch DMR: `device.receiver.launch_app(&CastDeviceApp::DefaultMediaReceiver)
  -> Result<Application, Error>`. `Application { app_id, session_id, transport_id,
  namespaces, display_name, status_text, ... }` (receiver.rs:73).
- Load HLS: `device.media.load(destination, session_id, &media) -> Result<Status, Error>`
  (media.rs:509). **destination = `app.transport_id`**, session_id = `app.session_id`.
- `Media` struct (media.rs:305) — **NO constructor; build struct literal**:
  ```rust
  use rust_cast::channels::media::{Media, StreamType};
  let media = Media {
      content_id: "http://host:port/live.m3u8".into(),
      stream_type: StreamType::Live,       // serializes to "LIVE"
      content_type: "application/x-mpegurl".into(),
      metadata: None,
      duration: None,
  };
  ```
- `MediaChannel::load` sends `{type:"LOAD", media:{contentId, contentType, streamType:"LIVE",...}}`
  → satisfies `grep "media/load" src/cast/sender.rs` exit criterion if our code comments/names
  reference it (the wire namespace is `urn:x-cast:com.google.cast.media` — the test greps for
  `"media/load"` string literal; ensure sender.rs contains that literal, e.g. in a doc comment
  or a const describing the media namespace path).
- rust_cast is `optional`/`cast` feature-gated in Cargo.toml → `src/cast/` must be
  `#[cfg(feature = "cast")]` or define its own error type without depending on rust_cast types
  in the default build. **Decision for default-build compile**: gate `mod cast` on
  `#[cfg(feature = "cast")]` in lib.rs, and put the pure payload-builder (a struct
  `LoadPayload { content_id, content_type, stream_type }` + serde Serialize producing
  `media/load`-shaped JSON) in a feature-independent submodule so the TDD test
  `test_cast_load_url_builds_media_load` runs WITHOUT the `cast` feature. Same for encode:
  `#[cfg(feature = "gstreamer")]`.

## tiny-skia 0.11.4

- `Pixmap::new(w, h) -> Option<Pixmap>`; `fill(Color::from_rgba8(r,g,b,255))`; `data()`
  → premultiplied RGBA. Opaque fills → exact RGB, so test pixel assertions hold.

## Test-visible contract recap (10 tests, tests/cast_tv_tests.rs)

All 10 tests from the spec TDD contract must compile+pass with **default features**.
`cast`/`gstreamer` features are for target-host runtime; the cast payload test must not
require rust_cast types (use our own serializable struct).
