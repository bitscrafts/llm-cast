//! Terminal cell damage tracker (spec-02).
//!
//! Pure and dependency-free: given the previous and current contents of a
//! grid, say which cells changed. No terminal types, no rendering, no I/O —
//! the caller converts `alacritty_terminal` cells into `CellKey`/`CellContent`.
//!
//! Visible rows run `0` down to `-(rows-1)`, hence `row: i32`.

use std::collections::{HashMap, HashSet};

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
    /// last-write-wins; each key is reported at most once.
    pub fn diff(&mut self, cells: &[(CellKey, CellContent)]) -> Vec<CellKey> {
        let mut damaged = Vec::new();
        let mut seen: HashSet<CellKey> = HashSet::with_capacity(cells.len());

        for (key, content) in cells {
            let changed = match self.previous.get(key) {
                Some(prev) => *prev != *content,
                None => true,
            };
            // Last-write-wins on the stored state, regardless of reporting.
            self.previous.insert(*key, *content);
            let first_seen = seen.insert(*key);
            if changed && first_seen {
                damaged.push(*key);
            }
        }

        // Forget keys that are no longer part of the grid, so a reappearance
        // is treated as first-time damage (D5).
        self.previous.retain(|key, _| seen.contains(key));

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
