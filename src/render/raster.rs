//! Grid → RGBA buffer rasterizer (R3).
//!
//! Paints the canvas directly (byte writes, no tiny-skia): each cell is an
//! 8×8 tile, background-filled then glyph-stamped from
//! [`crate::render::font::FONT8X8_BASIC`] (bit 0 = leftmost pixel column of
//! each glyph row — Hepper's font8x8_basic convention), tinted with the
//! cell fg. A `full` frame repaints every
//! tile; a diff frame repaints only the tiles listed in `frame.cells`, so
//! the caller's buffer is left untouched elsewhere.

use crate::emu::{Cell, ScreenFrame};
use crate::render::font::FONT8X8_BASIC;

/// Tile size in pixels (both axes).
pub const TILE: usize = 8;

/// Paint `frame` into `buffer`, an RGBA canvas of exactly
/// `width*8 × height*8` pixels, stride `width*8*4`.
///
/// Never panics: if the buffer is too small for the canvas, it is left
/// untouched and the call is a no-op.
pub fn rasterize(frame: &ScreenFrame, buffer: &mut [u8]) {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = width * TILE * 4;
    let canvas = stride * height * TILE;
    if buffer.len() < canvas {
        return;
    }

    for (cell, &(col, row)) in frame.cells.iter().zip(frame.positions.iter()) {
        paint_tile(buffer, stride, col as usize, row as usize, cell);
    }
}

/// Fill one 8×8 tile with the cell bg, then stamp the glyph pixels with fg.
fn paint_tile(buffer: &mut [u8], stride: usize, col: usize, row: usize, cell: &Cell) {
    let x0 = col * TILE;
    let y0 = row * TILE;

    for y in 0..TILE {
        for x in 0..TILE {
            let i = (y0 + y) * stride + (x0 + x) * 4;
            buffer[i] = cell.bg.r;
            buffer[i + 1] = cell.bg.g;
            buffer[i + 2] = cell.bg.b;
            buffer[i + 3] = 255;
        }
    }

    // Glyphs beyond the 128-entry table (e.g. wide/unicode) render as blank.
    let Some(glyph) = FONT8X8_BASIC.get(cell.ch as usize) else {
        return;
    };
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..TILE {
            // Bit 0 = leftmost pixel: Hepper's font8x8_basic stores each glyph
            // row with the leftmost column in the LSB, so gx=0 reads bit 0.
            // (Reading MSB-first — 0x80 >> gx — flips every glyph horizontally:
            // milestone-2 operator-test finding, 2026-08-16.)
            if bits & (1 << gx) != 0 {
                let i = (y0 + gy) * stride + (x0 + gx) * 4;
                buffer[i] = cell.fg.r;
                buffer[i + 1] = cell.fg.g;
                buffer[i + 2] = cell.fg.b;
                buffer[i + 3] = 255;
            }
        }
    }
}
