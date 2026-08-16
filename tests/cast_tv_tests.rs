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
