//! Capture side (R1): raw terminal bytes → vte emulator bridge.

pub mod bridge;
pub mod pipe;

pub use pipe::PipeSource;
