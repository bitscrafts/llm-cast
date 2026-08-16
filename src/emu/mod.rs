//! Terminal emulator core (spec 01 part 1): vte parse → grid → diff frames.
//!
//! The grid is row-major (`index = row * width + col`, row 0 = top visible
//! row). Frames are produced by diffing the grid against the previous state
//! with [`crate::damage::DamageTracker`].

pub mod term;

pub use term::{Cell, Rgb, ScreenFrame};
