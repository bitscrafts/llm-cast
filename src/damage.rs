//! Terminal cell damage tracker (spec-02).
//!
//! Pure and dependency-free: given the previous and current contents of a
//! grid, say which cells changed. No terminal types, no rendering, no I/O —
//! the caller converts `alacritty_terminal` cells into `CellKey`/`CellContent`.
//!
//! Visible rows run `0` down to `-(rows-1)`, hence `row: i32`.

use std::collections::HashMap;

/// Identifies one cell in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub row: i32,
    pub col: usize,
}

/// The content of one cell, reduced to what the renderer needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellContent {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
    pub flags: u8,
}

/// Owns the diff between consecutive grid contents.
///
/// Key decision: the tracker owns a `HashMap<CellKey, CellContent>` and
/// nothing else, so a change to `alacritty_terminal` cannot break it.
pub struct DamageTracker {
    previous: HashMap<CellKey, CellContent>,
}

impl DamageTracker {
    /// A fresh tracker: the next `diff` damages every key.
    pub fn new() -> Self {
        DamageTracker {
            previous: HashMap::new(),
        }
    }

    /// Compare `cells` against the previous call.
    ///
    /// Returns the keys whose content differs, plus keys seen for the first
    /// time, sorted by row descending then col ascending (deterministic).
    ///
    /// Keys absent from the current call are forgotten: a later reappearance
    /// counts as first-time damage. Duplicate keys within one call are
    /// last-write-wins: each key is reported at most once, and damage is
    /// judged on the surviving (final) value — an unchanged first occurrence
    /// never masks a changed later one (D2d).
    pub fn diff(&mut self, cells: &[(CellKey, CellContent)]) -> Vec<CellKey> {
        // Last-write-wins: collapse duplicates to the final per-key value
        // before comparing, so reporting and stored state agree (D2d).
        let mut current: HashMap<CellKey, CellContent> = HashMap::with_capacity(cells.len());
        for (key, content) in cells {
            current.insert(*key, *content);
        }

        let mut damaged = Vec::new();
        for (key, content) in &current {
            match self.previous.get(key) {
                Some(prev) if prev == content => {}
                _ => damaged.push(*key),
            }
        }

        // Replace state wholesale: keys absent from this call are forgotten,
        // so a reappearance counts as first-time damage (D5).
        self.previous = current;

        // Deterministic output order (D6): row descending, col ascending.
        damaged.sort_by(|a, b| b.row.cmp(&a.row).then(a.col.cmp(&b.col)));
        damaged
    }

    /// Forget all state, so the next `diff` damages everything (D7).
    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        DamageTracker::new()
    }
}
