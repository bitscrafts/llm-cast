//! Integration tests for the terminal cell damage tracker (spec-02).
//!
//! The tracker is pure and dependency-free: no terminal types, no rendering,
//! no I/O. Given the previous and current contents of a grid, it says which
//! cells changed.

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

    // The glyph ('X' = 0x58) stamps only its set bits, tinted fg.
    let glyph = FONT8X8_BASIC['X' as usize];
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..8 {
            let set = bits & (0x80 >> gx) != 0;
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
