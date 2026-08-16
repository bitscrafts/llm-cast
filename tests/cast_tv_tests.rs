//! Integration tests for cast-tv-terminal (spec-01 / spec-02).
//!
//! The first block covers the cell damage tracker (spec-02), which is pure
//! and dependency-free: no terminal types, no rendering, no I/O. Given the
//! previous and current contents of a grid, it says which cells changed.
//! Later blocks cover spec-01 parts 1-3: the vte emulator, the rasterizer,
//! the capture bridge, the cast sender, and the HLS server.

use cast_tv_terminal::damage::{CellContent, CellKey, DamageTracker};

fn cell(row: i32, col: usize, ch: char) -> (CellKey, CellContent) {
    (
        CellKey { row, col },
        CellContent {
            ch,
            fg: 0x00ff00,
            bg: 0x000000,
            flags: 0,
        },
    )
}

fn keys(cells: &[(CellKey, CellContent)]) -> Vec<CellKey> {
    cells.iter().map(|(k, _)| *k).collect()
}

/// Assert set equality regardless of order (order itself is D6's test).
fn assert_same_keys(actual: &[CellKey], expected: &[CellKey]) {
    let mut a = actual.to_vec();
    let mut e = expected.to_vec();
    let order = |x: &CellKey, y: &CellKey| y.row.cmp(&x.row).then(x.col.cmp(&y.col));
    a.sort_by(order);
    e.sort_by(order);
    assert_eq!(a, e);
}

/// D3 — on the first `diff`, every supplied key is damaged.
#[test]
fn test_first_diff_damages_everything() {
    let mut tracker = DamageTracker::new();
    let cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let damaged = tracker.diff(&cells);
    assert_same_keys(&damaged, &keys(&cells));
}

/// D4 — calling `diff` twice with identical input returns an empty Vec the
/// second time.
#[test]
fn test_identical_second_call_is_empty() {
    let mut tracker = DamageTracker::new();
    let cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let first = tracker.diff(&cells);
    assert_same_keys(&first, &keys(&cells));
    assert_eq!(tracker.diff(&cells), Vec::new());
}

/// D2a — a change of `ch` on one cell damages exactly that key.
#[test]
fn test_changed_char_is_damaged() {
    let mut tracker = DamageTracker::new();
    let mut cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&cells);

    cells[1].1.ch = 'Z';
    let damaged = tracker.diff(&cells);
    assert_eq!(damaged, vec![CellKey { row: -1, col: 2 }]);
}

/// D2b — a change of `fg` on one cell damages exactly that key.
#[test]
fn test_changed_colour_is_damaged() {
    let mut tracker = DamageTracker::new();
    let mut cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&cells);

    cells[2].1.fg = 0xff0000;
    let damaged = tracker.diff(&cells);
    assert_eq!(damaged, vec![CellKey { row: 2, col: 5 }]);
}

/// D2c — a new key appearing in the input is damaged.
#[test]
fn test_new_key_is_damaged() {
    let mut tracker = DamageTracker::new();
    let mut cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&cells);

    cells.push(cell(1, 3, 'd'));
    let damaged = tracker.diff(&cells);
    assert_eq!(damaged, vec![CellKey { row: 1, col: 3 }]);
}

/// D2d — damage is judged on the *final* per-key value: with previous
/// storing 'a' at K, a call [(K,'a'),(K,'b')] must report K exactly once —
/// the unchanged first occurrence must not mask the changed later one.
#[test]
fn test_duplicate_last_occurrence_change_is_damaged() {
    let mut tracker = DamageTracker::new();
    let previous = vec![cell(0, 0, 'a')];
    let _ = tracker.diff(&previous);

    let call = vec![cell(0, 0, 'a'), cell(0, 0, 'b')];
    let damaged = tracker.diff(&call);
    assert_eq!(damaged, vec![CellKey { row: 0, col: 0 }]);
}

/// D5a — a key present in the previous call and absent from the current one
/// is not reported as damaged.
#[test]
fn test_removed_key_is_not_damaged() {
    let mut tracker = DamageTracker::new();
    let cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&cells);

    let shrunk = vec![cell(0, 0, 'a'), cell(-1, 2, 'b')];
    assert_eq!(tracker.diff(&shrunk), Vec::new());
}

/// D5b (acceptance test) — a removed cell is forgotten, so its reappearance
/// counts as first-time damage. The naive implementation that keeps stale
/// entries forever fails this: the reappearing cell would compare equal to
/// its long-gone value and never repaint.
#[test]
fn test_reappearing_key_is_damaged() {
    let mut tracker = DamageTracker::new();
    let full = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&full);

    let shrunk = vec![cell(0, 0, 'a'), cell(2, 5, 'c')];
    let _ = tracker.diff(&shrunk);

    let damaged = tracker.diff(&full);
    assert_eq!(damaged, vec![CellKey { row: -1, col: 2 }]);
}

/// D6 — output is sorted by row descending then col ascending, and is
/// identical across two independent runs (HashMap iteration order must not
/// leak into the result).
#[test]
fn test_order_is_deterministic() {
    let mut v1: Vec<(CellKey, CellContent)> = Vec::new();
    for row in -2..=2 {
        for col in 0..=3 {
            v1.push(cell(row, col, 'x'));
        }
    }
    assert_eq!(v1.len(), 20);

    // Change five cells spread across rows/cols (index = (row+2)*4 + col).
    v1[0].1.ch = 'A'; // row -2, col 0
    v1[5].1.ch = 'B'; // row -1, col 1
    v1[9].1.ch = 'C'; // row 0,  col 1
    v1[13].1.ch = 'D'; // row 1,  col 1
    v1[19].1.ch = 'E'; // row 2,  col 3

    // v2 re-edits those five cells to different values, plus two more cells,
    // so v2 differs from v1 in exactly 7 cells spread across rows/cols.
    let mut v2 = v1.clone();
    v2[0].1.ch = 'a'; // row -2, col 0
    v2[5].1.ch = 'b'; // row -1, col 1
    v2[9].1.ch = 'c'; // row 0,  col 1
    v2[13].1.ch = 'd'; // row 1,  col 1
    v2[19].1.ch = 'e'; // row 2,  col 3
    v2[3].1.ch = 'Q'; // row -2, col 3
    v2[16].1.bg = 0x112233; // row 2,  col 0

    let run = |before: &[(CellKey, CellContent)], after: &[(CellKey, CellContent)]| {
        let mut tracker = DamageTracker::new();
        let _ = tracker.diff(before);
        tracker.diff(after)
    };

    let first = run(&v1, &v2);
    let second = run(&v1, &v2);
    assert_eq!(first, second);

    // Sorted row desc, col asc — exact order.
    let expected = [
        CellKey { row: 2, col: 0 },
        CellKey { row: 2, col: 3 },
        CellKey { row: 1, col: 1 },
        CellKey { row: 0, col: 1 },
        CellKey { row: -1, col: 1 },
        CellKey { row: -2, col: 0 },
        CellKey { row: -2, col: 3 },
    ];
    assert_eq!(first, expected);

    // Also assert the ordering invariant directly.
    assert!(first
        .windows(2)
        .all(|w| w[0].row > w[1].row || (w[0].row == w[1].row && w[0].col < w[1].col)));
}

/// D7 — `reset` forgets all state, so the next diff damages everything.
#[test]
fn test_reset_damages_everything() {
    let mut tracker = DamageTracker::new();
    let cells = vec![cell(0, 0, 'a'), cell(-1, 2, 'b'), cell(2, 5, 'c')];
    let _ = tracker.diff(&cells);
    assert_eq!(tracker.diff(&cells), Vec::new());

    tracker.reset();
    let damaged = tracker.diff(&cells);
    assert_same_keys(&damaged, &keys(&cells));
}

/// Error-handling expectation — duplicate keys in a single call are
/// last-write-wins, not a panic: the key is reported at most once and the
/// surviving stored value is the last occurrence.
#[test]
fn test_duplicate_key_last_write_wins() {
    let mut tracker = DamageTracker::new();
    let dupes = vec![
        cell(0, 0, 'a'),
        cell(0, 0, 'b'), // overwrites the first occurrence
    ];
    let damaged = tracker.diff(&dupes);
    assert_eq!(damaged, vec![CellKey { row: 0, col: 0 }]);

    // The stored value is the last write: 'b' alone compares equal.
    let surviving = vec![cell(0, 0, 'b')];
    assert_eq!(tracker.diff(&surviving), Vec::new());
}

// ---------------------------------------------------------------------------
// spec 01 part 1 (R2/R3/R7): vte parser → grid, rasterizer.
// TDD Contract tests — written before the emulator/rasterizer existed.
// ---------------------------------------------------------------------------

use cast_tv_terminal::emu::term::{Emulator, DEFAULT_FG};
use cast_tv_terminal::emu::{Cell, Rgb, ScreenFrame};
use cast_tv_terminal::render::font::FONT8X8_BASIC;
use cast_tv_terminal::render::raster::rasterize;

/// T1 — `"\x1b[31mX\x1b[0mY"`: SGR red paints X red, SGR reset returns Y to
/// the default fg; neither cell is bold.
#[test]
fn test_vte_parses_ansi_into_grid() {
    let mut emu = Emulator::with_size(3, 2);
    let frame = emu.parse_bytes(b"\x1b[31mX\x1b[0mY");

    assert!(frame.full);
    assert_eq!(frame.cells.len(), 3 * 2);

    let x = frame.cells[0];
    assert_eq!(x.ch, 'X');
    assert_eq!(x.fg, Rgb { r: 255, g: 0, b: 0 });
    assert!(!x.bold);

    let y = frame.cells[1];
    assert_eq!(y.ch, 'Y');
    assert_eq!(y.fg, DEFAULT_FG);
    assert!(!y.bold);
}

/// T2 — the first `parse_bytes` on a fresh emulator returns a full frame:
/// `full == true` and `cells.len() == width * height`.
#[test]
fn test_first_frame_is_full() {
    let mut emu = Emulator::with_size(3, 2);
    let frame = emu.parse_bytes(b"abc");

    assert!(frame.full);
    assert_eq!(frame.cells.len(), 3 * 2);
}

/// T3 — after a first call, a later call that changes a single cell yields a
/// diff frame: `full == false` and `cells` holds exactly that changed cell.
#[test]
fn test_subsequent_frames_are_diff() {
    let mut emu = Emulator::with_size(4, 2);
    let _ = emu.parse_bytes(b"abcd"); // region: row 0, cols 0..=3
    let frame = emu.parse_bytes(b"abXd"); // only col 2 changes

    assert!(!frame.full);
    assert_eq!(frame.cells.len(), 1);
    assert_eq!(frame.cells[0].ch, 'X');
    assert_eq!(frame.positions[0], (2, 0));
}

/// T3b — CHA (CSI `n G`) moves the cursor horizontally without touching the
/// row, so text lands at an absolute column (ncurses optimization — htop
/// uses it for every field).
#[test]
fn test_cha_moves_column_only() {
    let mut emu = Emulator::with_size(8, 3);
    let frame = emu.parse_bytes(b"\x1b[6GX");
    assert_eq!(frame.cells[5].ch, 'X'); // 1-based col 6 = index 5
    assert_eq!(frame.cells[0].ch, ' '); // col 1 untouched
}

/// T3c — VPA (CSI `n d`) moves the cursor vertically without touching the
/// column (htop's other single-axis positioning primitive).
#[test]
fn test_vpa_moves_row_only() {
    let mut emu = Emulator::with_size(8, 3);
    let frame = emu.parse_bytes(b"\x1b[3dX");
    assert_eq!(frame.cells[2 * 8].ch, 'X'); // 1-based row 3 = index 2
}

/// T3d — CNL (CSI `n E`) moves down n rows to column 0; CPL (CSI `n F`)
/// moves back up.
#[test]
fn test_cnl_cpl() {
    let mut emu = Emulator::with_size(4, 4);
    let frame = emu.parse_bytes(b"\x1b[2EX\x1b[2FX");
    assert_eq!(frame.cells[2 * 4].ch, 'X'); // CNL 2 → row 2, col 0
    assert_eq!(frame.cells[0].ch, 'X'); // CPL 2 → row 0, col 0
}

/// T3e — EL (CSI `K`): mode 2 blanks the whole current line.
#[test]
fn test_el_erases_line() {
    // 8 wide so "abcd" doesn't wrap the cursor to row 1 (the erase targets
    // the cursor's row, exactly like a real terminal).
    let mut emu = Emulator::with_size(8, 2);
    let frame = emu.parse_bytes(b"abcd\x1b[2K");
    for c in &frame.cells[0..8] {
        assert_eq!(c.ch, ' ');
    }
}

/// T3f — ED (CSI `J`): mode 2 blanks the whole grid (the startup
/// `\x1b[H\x1b[2J` sequence every full-screen app emits).
#[test]
fn test_ed_erases_display() {
    let mut emu = Emulator::with_size(3, 2);
    let frame = emu.parse_bytes(b"abcdef\x1b[2J");
    for c in &frame.cells {
        assert_eq!(c.ch, ' ');
    }
}

/// T4 — rasterize a 2×2 grid with known fg/bg and one non-space glyph: the
/// buffer is exactly `width*8 * height*8 * 4` RGBA bytes, bg fills every
/// tile, and glyph pixels are tinted fg.
#[test]
fn test_rasterize_grid_to_buffer() {
    let fg = Rgb { r: 255, g: 0, b: 0 };
    let bg = Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    let blank = Cell {
        ch: ' ',
        fg,
        bg,
        bold: false,
    };
    let frame = ScreenFrame {
        width: 2,
        height: 2,
        cells: vec![
            Cell {
                ch: 'X',
                fg,
                bg,
                bold: false,
            },
            blank,
            blank,
            blank,
        ],
        positions: vec![(0, 0), (1, 0), (0, 1), (1, 1)],
        full: true,
    };

    let w = 2 * 8;
    let h = 2 * 8;
    let mut buffer = vec![0u8; w * h * 4];
    rasterize(&frame, &mut buffer);

    assert_eq!(buffer.len(), w * h * 4);

    let px = |x: usize, y: usize| &buffer[(y * w + x) * 4..(y * w + x) * 4 + 4];

    // Every pixel is exactly fg or bg, opaque — nothing else.
    for y in 0..h {
        for x in 0..w {
            let p = px(x, y);
            let is_fg = p == [255, 0, 0, 255];
            let is_bg = p == [10, 20, 30, 255];
            assert!(
                is_fg || is_bg,
                "pixel ({x},{y}) = {p:?} is neither fg nor bg"
            );
        }
    }

    // The glyph ('X' = 0x58) stamps only its set bits, tinted fg. Hepper's
    // font8x8_basic stores bit 0 = leftmost pixel, so the rasterizer reads
    // LSB-first (1 << gx); MSB-first would mirror the glyph.
    let glyph = FONT8X8_BASIC['X' as usize];
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..8 {
            let set = bits & (1 << gx) != 0;
            let expect = if set {
                [255, 0, 0, 255]
            } else {
                [10, 20, 30, 255]
            };
            assert_eq!(px(gx, gy), &expect, "glyph pixel ({gx},{gy})");
        }
    }

    // A full frame leaves no tile unpainted: the far tile is all bg.
    for y in 8..h {
        for x in 8..w {
            assert_eq!(px(x, y), &[10, 20, 30, 255]);
        }
    }
}

// ===========================================================================
// spec-02 part 2 (R1 capture bridge, R6 cast sender)
// ===========================================================================

use cast_tv_terminal::capture::bridge::{Bridge, BridgeError, ByteSource};
use cast_tv_terminal::cast::{CastError, DeviceAddr, Sender};

/// In-memory byte source: hands out the whole payload in one read, then EOF.
struct FakeByteSource {
    data: Vec<u8>,
    pos: usize,
}

impl ByteSource for FakeByteSource {
    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, BridgeError> {
        let remaining = self.data.len().saturating_sub(self.pos);
        let n = remaining.min(buf.len());
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// R1 — `Bridge::poll` feeds source bytes through the vte emulator, so the
/// screen advances from blank after the poll.
#[test]
fn test_capture_bridge_feeds_bytes_to_vte() {
    // SGR 31 (red) + "HELLO": a VT sequence that must land on the grid.
    let source = FakeByteSource {
        data: b"\x1b[31mHELLO".to_vec(),
        pos: 0,
    };
    let mut bridge = Bridge::new(source, Emulator::new());

    let fed = bridge.poll().unwrap();
    assert_eq!(fed, b"\x1b[31mHELLO".len());

    let frame = bridge.frame();
    let hello = frame.cells.iter().find(|cell| cell.ch == 'H');
    assert!(hello.is_some(), "emulator grid must contain the fed text");
    assert_eq!(
        hello.map(|cell| cell.fg),
        Some(Rgb { r: 255, g: 0, b: 0 }),
        "fed text must carry the SGR color"
    );
}

/// R6 — the media/load payload is a Cast v2 LOAD request carrying the HLS
/// URL under `media`.
#[test]
fn test_cast_load_url_builds_media_load() {
    let url = "http://tv:8080/live.m3u8";
    let payload = Sender::build_media_load_request(url);
    assert_eq!(payload["type"], "LOAD");
    assert_eq!(payload["media"]["contentId"], url);
    assert_eq!(payload["media"]["streamType"], "LIVE");
}

/// R6 — an injected discovery that always fails surfaces a clear
/// `CastError::Unreachable` promptly, with no network access.
#[test]
fn test_sender_reports_unreachable() {
    let mut sender = Sender::new(Box::new(|| {
        Err(CastError::Unreachable(
            "no chromecast found on the LAN".into(),
        ))
    }));
    let result = sender.send_load("http://tv:8080/live.m3u8");
    match result {
        Err(CastError::Unreachable(_)) => {}
        other => panic!("expected CastError::Unreachable, got {other:?}"),
    }
}

/// R6 — an injected discovery that yields a device address composes the
/// amended seam end-to-end: under default features the rust_cast session is
/// compiled out, so `send_load(url)` returns `Ok(())` with no network
/// touched.
///
/// Default features ONLY: under `--features cast` the session is live, so an
/// injected `Ok(DeviceAddr)` would trigger a real network connect to the fake
/// address. The smoke-test remediation on 2026-08-16 gated this test to
/// `#[cfg(not(feature = "cast"))]`; the live path is covered by the real
/// device smoke test via `castctl`.
#[test]
#[cfg(not(feature = "cast"))]
fn test_sender_accepts_device_address() {
    let mut sender = Sender::new(Box::new(|| {
        Ok(DeviceAddr {
            host: "192.168.1.50".into(),
            port: 8009,
        })
    }));
    let result = sender.send_load("http://tv:8080/live.m3u8");
    assert!(result.is_ok());
}

/// R6 — `DeviceAddr::new` pins the standard Chromecast port 8009.
#[test]
fn test_device_addr_default_port() {
    let addr = DeviceAddr::new("10.0.0.5");
    assert_eq!(addr.port, 8009);
    assert_eq!(addr.host, "10.0.0.5");
}

// ===========================================================================
// spec-01 part 3 (R5 HLS server + CORS, R4 H.264 encode module)
// ===========================================================================
//
// TDD Contract tests (parent tests 5 and 6) — written before the server
// existed. A raw HTTP/1.1 GET over a TcpStream stands in for the
// Default Media Receiver's fetch, so no client dependency is needed.

use cast_tv_terminal::serve::server;
use cast_tv_terminal::serve::store::{MapStore, MediaStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// One raw HTTP/1.1 GET with an `Origin` header (as a browser/Chromecast
/// sender would send). Returns (status, lowercased-header map, body).
async fn raw_get(addr: &str, path: &str) -> (u16, HashMap<String, String>, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nOrigin: http://tv.local\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();

    let head_end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&response[..head_end]);
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    (status, headers, response[head_end + 4..].to_vec())
}

/// Boot the real HLS router over a store on an ephemeral port; returns its
/// address.
async fn spawn_server(store: Arc<dyn MediaStore>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        axum::serve(listener, server::app(store)).await.unwrap();
    });
    addr
}

/// T5/T6 (R5, reworked in spec-01 part 6) — the HLS router reads LIVE from
/// the store instead of static stand-ins: GET /live.m3u8 returns 200 with
/// the seeded playlist text, GET /segment/<name> returns 200 with the
/// seeded segment bytes, an unknown segment is 404, and the CORS header is
/// present on both 200s (the part-3 assertions are preserved).
#[tokio::test]
async fn test_served_playlist_reads_from_store() {
    let playlist_text =
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:1\n#EXTINF:1.0,\nseg0.ts\n";
    let store: Arc<dyn MediaStore> = Arc::new(MapStore::seeded(
        playlist_text,
        "seg0.ts",
        b"SEG0-BYTES-0000000000000000".to_vec(),
    ));
    let addr = spawn_server(store).await;

    let (status, headers, body) = raw_get(&addr, "/live.m3u8").await;
    assert_eq!(status, 200);
    assert!(
        headers.contains_key("access-control-allow-origin"),
        "playlist response must carry Access-Control-Allow-Origin, got headers {headers:?}"
    );
    assert_eq!(body, playlist_text.as_bytes());

    let (status, headers, body) = raw_get(&addr, "/segment/seg0.ts").await;
    assert_eq!(status, 200);
    assert!(
        headers.contains_key("access-control-allow-origin"),
        "segment response must carry Access-Control-Allow-Origin, got headers {headers:?}"
    );
    assert_eq!(body, b"SEG0-BYTES-0000000000000000");

    let (status, _, _) = raw_get(&addr, "/segment/unknown.ts").await;
    assert_eq!(status, 404);
}

/// P7 — `serve_hls(store, listener)` serves the store-backed router on a
/// caller-provided bound listener (tests bind `127.0.0.1:0`).
#[tokio::test]
async fn test_serve_hls_binds_and_responds() {
    let playlist_text =
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:1\n#EXTINF:1.0,\nseg0.ts\n";
    let store: Arc<dyn MediaStore> = Arc::new(MapStore::seeded(
        playlist_text,
        "seg0.ts",
        b"SEG0-BYTES-0000000000000000".to_vec(),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        server::serve_hls(store, listener).await;
    });

    let (status, headers, body) = raw_get(&addr, "/live.m3u8").await;
    assert_eq!(status, 200);
    assert!(headers.contains_key("access-control-allow-origin"));
    assert!(body.starts_with(b"#EXTM3U"));
}

// ===========================================================================
// spec-01 part 4 (R8-R11 closing sweep) — no production unwraps
// ===========================================================================

use std::path::PathBuf;

/// R8 — no production file under the module dirs (six pipeline modules, plus
/// the spec-03 `src/mcp` and `src/mux` since part 1) may call `.unwrap()` or
/// `.expect()` on a non-test, non-comment line. Panic points in a pipeline
/// that must degrade safely are forbidden; a panic in the MCP server is a
/// crashed stdio session.
#[test]
fn test_no_production_unwrap() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let module_dirs = [
        "src/capture",
        "src/emu",
        "src/render",
        "src/encode",
        "src/serve",
        "src/cast",
        "src/mcp",
        "src/mux",
    ];

    let mut checked = 0usize;
    let mut checked_dirs: Vec<String> = Vec::new();
    let mut offenders: Vec<(String, usize, String)> = Vec::new();
    let mut missing_dirs: Vec<String> = Vec::new();

    for dir in module_dirs {
        let path = manifest.join(dir);
        if !path.is_dir() {
            missing_dirs.push(dir.to_string());
            continue;
        }
        for entry in std::fs::read_dir(&path).expect("read_dir must succeed") {
            let entry = entry.expect("dir entry must be readable");
            let file_path = entry.path();
            if file_path.extension().map(|e| e == "rs").unwrap_or(false) {
                let content = std::fs::read_to_string(&file_path).expect("source must be readable");
                // Find every `.unwrap()`/`.expect()` call, rejecting lines
                // that are blank or comments (doc comments included).
                for (lineno, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with("//") {
                        continue;
                    }
                    if trimmed.contains(".unwrap()") || trimmed.contains(".expect()") {
                        offenders.push((
                            file_path
                                .strip_prefix(&manifest)
                                .unwrap_or(&file_path)
                                .display()
                                .to_string(),
                            lineno + 1,
                            trimmed.to_string(),
                        ));
                    }
                }
                checked += 1;
                if !checked_dirs.contains(&dir.to_string()) {
                    checked_dirs.push(dir.to_string());
                }
            }
        }
    }

    assert!(
        !missing_dirs.is_empty() || checked > 0,
        "no module dirs found to check; the walk must not pass vacuously"
    );
    assert!(
        missing_dirs.is_empty(),
        "production module dirs missing: {missing_dirs:?}"
    );
    assert_eq!(
        checked_dirs.len(),
        8,
        "all eight module dirs must have been walked, got {checked_dirs:?}"
    );
    assert!(
        offenders.is_empty(),
        "production code must not call .unwrap()/.expect(), found: {offenders:?}"
    );
}

// ===========================================================================
// spec-01 part 6 (full pipeline integration, default features): PipeSource,
// NullEncoder + Encode trait, the pipeline coordinator, and the artifact
// stores. Written before the production code per the TDD contract.
// ===========================================================================

use cast_tv_terminal::capture::pipe::PipeSource;
use cast_tv_terminal::encode::pipe::{Encode, NullEncoder};
use cast_tv_terminal::pipeline::{Pipeline, PipelineConfig};
use cast_tv_terminal::serve::store::DirStore;

/// Unique throwaway directory under the OS temp dir (no tempfile dep).
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("cast-tv-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// P1 — `PipeSource::open` on a file already holding `b"hi\x1b[31m"` (5
/// bytes): the first `read_available` copies them all, and a second read at
/// the current EOF returns 0 — the never-blocking `ByteSource` contract.
#[test]
fn test_pipe_source_reads_available_bytes() {
    let dir = scratch_dir("pipe-source");
    let path = dir.join("pane.out");
    std::fs::write(&path, b"hi\x1b[31m").unwrap();

    let mut source = PipeSource::open(&path).unwrap();
    let mut buf = [0u8; 64];
    // The file holds 7 bytes: "hi" (2) + "\x1b[31m" (5).
    let n = source.read_available(&mut buf).unwrap();
    assert_eq!(n, 7);
    assert_eq!(&buf[..7], b"hi\x1b[31m");

    let n = source.read_available(&mut buf).unwrap();
    assert_eq!(n, 0, "a second read at EOF must return 0");

    let _ = std::fs::remove_dir_all(&dir);
}

/// P2 — `NullEncoder` counts submissions and reports the stream URL.
#[test]
fn test_null_encoder_counts_frames() {
    let url = "http://h:8080/live.m3u8";
    let mut encoder = NullEncoder::new(url.to_string());
    for _ in 0..3 {
        encoder.submit_frame(&[0u8; 4], 8, 8).unwrap();
    }
    assert_eq!(encoder.submitted(), 3);
    assert_eq!(encoder.stream_url(), url);
}

/// A 3×2 emulator fed `"\x1b[31mHELLO"` through the same FakeByteSource the
/// part-2 bridge test uses, inside the real coordinator loop.
fn sample_pipeline() -> Pipeline<FakeByteSource, NullEncoder> {
    let source = FakeByteSource {
        data: b"\x1b[31mHELLO".to_vec(),
        pos: 0,
    };
    let bridge = Bridge::new(source, Emulator::with_size(3, 2));
    Pipeline::new(
        bridge,
        NullEncoder::new("http://h:8080/live.m3u8".to_string()),
        PipelineConfig::default(),
    )
}

/// P3 — a changed frame (new bytes through the bridge) is submitted, and
/// the encoder saw the emu grid size × 8 canvas (3×2 grid → 24×16).
#[test]
fn test_pipeline_submits_changed_frames() {
    let mut pipeline = sample_pipeline();
    pipeline.poll_and_submit(0).unwrap();
    assert!(pipeline.encoder().submitted() >= 1);
    assert_eq!(pipeline.encoder().last_dims(), (3 * 8, 2 * 8));
}

/// P4 — with no new bytes and the keepalive not yet due, a second step
/// submits nothing: unchanged diff frames are skipped.
#[test]
fn test_pipeline_skips_unchanged_frames() {
    let mut pipeline = sample_pipeline();
    pipeline.poll_and_submit(0).unwrap();
    let first = pipeline.encoder().submitted();
    pipeline.poll_and_submit(10).unwrap();
    assert_eq!(pipeline.encoder().submitted(), first);
}

/// P5 — an unchanged screen past the keepalive deadline still produces
/// exactly one more submission (the keepalive frame) so HLS keeps flowing.
#[test]
fn test_pipeline_keepalive_after_idle() {
    let mut pipeline = sample_pipeline();
    pipeline.poll_and_submit(0).unwrap();
    let first = pipeline.encoder().submitted();
    pipeline.poll_and_submit(1001).unwrap();
    assert_eq!(pipeline.encoder().submitted(), first + 1);
    assert_eq!(pipeline.encoder().last_dims(), (3 * 8, 2 * 8));
}

/// P6 — `DirStore` reads a live hlssink2-style output dir: playlist text,
/// segment bytes, and None for unknown names.
#[test]
fn test_dir_store_reads_output_dir() {
    let dir = scratch_dir("dir-store");
    std::fs::write(
        dir.join("live.m3u8"),
        "#EXTM3U\n#EXTINF:1.0,\nseg_00000.ts\n",
    )
    .unwrap();
    // hlssink2 layout: playlist in `dir/`, segments in `dir/segment/`.
    std::fs::create_dir_all(dir.join("segment")).unwrap();
    std::fs::write(dir.join("segment").join("seg_00000.ts"), b"SEGMENT-00000").unwrap();

    let store = DirStore::new(&dir);
    assert_eq!(
        store.playlist(),
        Some("#EXTM3U\n#EXTINF:1.0,\nseg_00000.ts\n".to_string())
    );
    assert_eq!(
        store.segment("seg_00000.ts"),
        Some(b"SEGMENT-00000".to_vec())
    );
    assert_eq!(store.segment("missing.ts"), None);

    let _ = std::fs::remove_dir_all(&dir);
}
