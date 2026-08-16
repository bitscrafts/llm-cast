//! vte parser → grid → diffed `ScreenFrame`s (R2, R7).
//!
//! Implements [`vte::Perform`] (via `alacritty_terminal::vte`, which
//! re-exports vte 0.13 — no new dependency) for the minimum the pipeline
//! needs: `print`, the C0 controls CR/LF/BS/HT, CUP and SGR. OSC and the
//! other dispatch hooks are no-ops.

use alacritty_terminal::vte::{self, Perform};

use crate::damage::{CellContent, CellKey, DamageTracker};

/// 24-bit RGB. Packed to `u32` as `0xRRGGBB` when talking to the damage
/// tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// One grid cell: glyph plus ANSI attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
}

impl Default for Cell {
    /// A blank cell with the ANSI default colors.
    fn default() -> Self {
        Cell {
            ch: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
        }
    }
}

/// ANSI default foreground: 192,192,192 — the classic light-gray terminal
/// text (documented choice, matches alacritty's default theme text color).
pub const DEFAULT_FG: Rgb = Rgb {
    r: 192,
    g: 192,
    b: 192,
};

/// ANSI default background: 0,0,0 — classic black terminal background.
pub const DEFAULT_BG: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// Basic ANSI 30-37/40-47 palette (red is 255,0,0 per the TDD contract).
const BASIC_COLORS: [Rgb; 8] = [
    Rgb { r: 0, g: 0, b: 0 },   // 30 black
    Rgb { r: 255, g: 0, b: 0 }, // 31 red
    Rgb { r: 0, g: 255, b: 0 }, // 32 green
    Rgb {
        r: 255,
        g: 255,
        b: 0,
    }, // 33 yellow
    Rgb { r: 0, g: 0, b: 255 }, // 34 blue
    Rgb {
        r: 255,
        g: 0,
        b: 255,
    }, // 35 magenta
    Rgb {
        r: 0,
        g: 255,
        b: 255,
    }, // 36 cyan
    Rgb {
        r: 255,
        g: 255,
        b: 255,
    }, // 37 white
];

/// Bright ANSI 90-97/100-107 palette (xterm-style: same hues, lighter).
const BRIGHT_COLORS: [Rgb; 8] = [
    Rgb {
        r: 128,
        g: 128,
        b: 128,
    }, // 90 bright black
    Rgb {
        r: 255,
        g: 85,
        b: 85,
    }, // 91 bright red
    Rgb {
        r: 85,
        g: 255,
        b: 85,
    }, // 92 bright green
    Rgb {
        r: 255,
        g: 255,
        b: 85,
    }, // 93 bright yellow
    Rgb {
        r: 85,
        g: 85,
        b: 255,
    }, // 94 bright blue
    Rgb {
        r: 255,
        g: 85,
        b: 255,
    }, // 95 bright magenta
    Rgb {
        r: 85,
        g: 255,
        b: 255,
    }, // 96 bright cyan
    Rgb {
        r: 255,
        g: 255,
        b: 255,
    }, // 97 bright white
];

/// One emitted frame.
///
/// `cells` holds the whole grid (row-major) when `full`; otherwise it holds
/// only the cells that changed since the previous frame. `positions` runs
/// parallel to `cells`: `(col, row)` of each cell, row 0 = top visible row.
#[derive(Debug, Clone)]
pub struct ScreenFrame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
    /// `(col, row)` per entry of `cells`; row 0 = top visible row.
    pub positions: Vec<(u16, u16)>,
    /// True iff this frame covers the whole grid (first frame on a fresh
    /// tracker); false on later diffs.
    pub full: bool,
}

/// A minimal terminal emulator: vte-driven grid plus a damage tracker.
pub struct Emulator {
    width: u16,
    height: u16,
    /// Row-major grid: `index = row * width + col`, row 0 = top.
    grid: Vec<Cell>,
    /// Cursor position, in cells.
    col: u16,
    row: u16,
    /// Active ANSI attributes.
    fg: Rgb,
    bg: Rgb,
    bold: bool,
    tracker: DamageTracker,
}

impl Emulator {
    /// A default 80×24 emulator.
    pub fn new() -> Self {
        Emulator::with_size(80, 24)
    }

    /// An emulator with the given size. The grid starts as blank cells with
    /// default colors, so the first emitted frame is full and deterministic.
    pub fn with_size(width: u16, height: u16) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Emulator {
            width,
            height,
            grid: vec![Cell::default(); width as usize * height as usize],
            col: 0,
            row: 0,
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            tracker: DamageTracker::new(),
        }
    }

    /// Feed raw bytes through the vte parser, then emit the frame for the
    /// resulting grid state.
    pub fn parse_bytes(&mut self, bytes: &[u8]) -> ScreenFrame {
        let mut parser = vte::Parser::new();
        for &byte in bytes {
            parser.advance(self, byte);
        }
        self.emit_frame()
    }

    /// Diff the grid against the tracker and build the frame.
    ///
    /// A fresh tracker damages every cell on its first `diff` (→ `full`);
    /// later calls return only changed cells (→ `full == false`).
    fn emit_frame(&mut self) -> ScreenFrame {
        let width = self.width as usize;
        let total = self.grid.len();

        let mut contents: Vec<(CellKey, CellContent)> = Vec::with_capacity(total);
        for (index, cell) in self.grid.iter().enumerate() {
            let row = index / width;
            let col = index % width;
            contents.push((
                CellKey {
                    row: -(row as i32),
                    col,
                },
                CellContent {
                    ch: cell.ch,
                    fg: rgb_to_u32(cell.fg),
                    bg: rgb_to_u32(cell.bg),
                    flags: u8::from(cell.bold),
                },
            ));
        }

        let damaged = self.tracker.diff(&contents);
        let full = damaged.len() == total;

        let (cells, positions) = if full {
            let positions = (0..total)
                .map(|i| ((i % width) as u16, (i / width) as u16))
                .collect();
            (self.grid.clone(), positions)
        } else {
            let mut cells = Vec::with_capacity(damaged.len());
            let mut positions = Vec::with_capacity(damaged.len());
            for key in &damaged {
                cells.push(self.grid[(-key.row as usize) * width + key.col]);
                positions.push((key.col as u16, (-key.row) as u16));
            }
            (cells, positions)
        };

        ScreenFrame {
            width: self.width,
            height: self.height,
            cells,
            positions,
            full,
        }
    }

    /// Scroll the grid up so the cursor row is back in view, blanking the
    /// freed rows at the bottom.
    fn scroll_into_view(&mut self) {
        let width = self.width as usize;
        let height = self.height as usize;
        while self.row as usize >= height {
            self.grid.drain(..width);
            self.grid
                .extend(std::iter::repeat_n(Cell::default(), width));
            self.row = self.height - 1;
        }
    }

    /// Reset ANSI attributes to the defaults (SGR 0).
    fn reset_attrs(&mut self) {
        self.fg = DEFAULT_FG;
        self.bg = DEFAULT_BG;
        self.bold = false;
    }

    /// CUP (CSI `row;col H`): move the cursor; both params are 1-based, 0
    /// means 1 (ECMA-48). Out-of-range values clamp to the grid edge.
    fn cup(&mut self, params: &vte::Params) {
        let mut iter = params.iter();
        let row_param = iter.next().and_then(|p| p.first()).copied().unwrap_or(1);
        let col_param = iter.next().and_then(|p| p.first()).copied().unwrap_or(1);
        let row = if row_param == 0 { 1 } else { row_param };
        let col = if col_param == 0 { 1 } else { col_param };
        self.row = (row - 1).min(self.height - 1);
        self.col = (col - 1).min(self.width - 1);
    }

    /// SGR (CSI `... m`): fg/bg color and bold. Unknown codes are ignored;
    /// out-of-range indices fall through to the `_` arm — never panic.
    fn sgr(&mut self, params: &vte::Params) {
        let codes: Vec<u16> = params.iter().filter_map(|p| p.first()).copied().collect();
        // CSI m with no params is SGR 0.
        if codes.is_empty() {
            self.reset_attrs();
            return;
        }
        for code in codes {
            match code {
                0 => self.reset_attrs(),
                1 => self.bold = true,
                22 => self.bold = false,
                30..=37 => self.fg = BASIC_COLORS[(code - 30) as usize],
                39 => self.fg = DEFAULT_FG,
                40..=47 => self.bg = BASIC_COLORS[(code - 40) as usize],
                49 => self.bg = DEFAULT_BG,
                90..=97 => self.fg = BRIGHT_COLORS[(code - 90) as usize],
                100..=107 => self.bg = BRIGHT_COLORS[(code - 100) as usize],
                _ => {}
            }
        }
    }
}

impl Default for Emulator {
    fn default() -> Self {
        Emulator::new()
    }
}

impl Perform for Emulator {
    /// Store the character at the cursor and advance; wrap at the right
    /// edge (with scrollback into view).
    fn print(&mut self, c: char) {
        let index = self.row as usize * self.width as usize + self.col as usize;
        self.grid[index] = Cell {
            ch: c,
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
        };
        self.col += 1;
        if self.col >= self.width {
            self.col = 0;
            self.row += 1;
            self.scroll_into_view();
        }
    }

    /// C0 controls: CR, LF, BS, HT; everything else is a no-op.
    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => self.col = 0,
            b'\n' => {
                self.row += 1;
                self.scroll_into_view();
            }
            b'\x08' if self.col > 0 => self.col -= 1,
            b'\t' => {
                self.col = (self.col / 8 + 1) * 8;
                if self.col >= self.width {
                    self.col = self.width - 1;
                }
            }
            _ => {}
        }
    }

    /// CSI dispatch: only CUP and SGR are handled; the rest are no-ops.
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'H' | 'f' => self.cup(params),
            'm' => self.sgr(params),
            _ => {}
        }
    }
}

/// Pack `Rgb` to the `0xRRGGBB` form the damage tracker compares.
fn rgb_to_u32(color: Rgb) -> u32 {
    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}
