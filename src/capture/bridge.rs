//! Capture bridge (R1): pump bytes from a [`ByteSource`] into the vte
//! emulator.
//!
//! The production source (a tmux-style pipe/socket feeding the `herdr` pane
//! output) is a later integration; this part ships the seam. [`Bridge::poll`]
//! drains whatever the source has available and feeds it through
//! [`Emulator::parse_bytes`], keeping the latest emitted [`ScreenFrame`].

use crate::emu::{Emulator, ScreenFrame};

/// Per-read buffer handed to the byte source.
const BUF_SIZE: usize = 4096;

/// A source of raw terminal bytes.
///
/// `read_available` copies whatever is currently available into `buf` and
/// returns the number of bytes copied (0 = nothing available right now). It
/// must not block waiting for data.
pub trait ByteSource {
    /// Copy available bytes into `buf`; returns the number copied.
    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, BridgeError>;
}

/// Errors surfaced by the capture bridge.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// The underlying byte source failed to read.
    #[error("byte source read failed: {0}")]
    Read(#[from] std::io::Error),
}

/// Pumps bytes from a [`ByteSource`] into an [`Emulator`].
pub struct Bridge<S: ByteSource> {
    source: S,
    emu: Emulator,
    /// Latest frame emitted by the emulator (blank at construction).
    last_frame: ScreenFrame,
}

impl<S: ByteSource> Bridge<S> {
    /// Wrap `source` around `emu`.
    ///
    /// Seeds `last_frame` with the emulator's initial (blank, full) frame so
    /// `frame()` is always the current screen.
    pub fn new(source: S, mut emu: Emulator) -> Self {
        let last_frame = emu.parse_bytes(&[]);
        Bridge {
            source,
            emu,
            last_frame,
        }
    }

    /// The latest screen state emitted by the emulator.
    pub fn frame(&self) -> &ScreenFrame {
        &self.last_frame
    }

    /// Feed all currently-available bytes from the source into the emulator.
    ///
    /// Returns the number of bytes fed. Reads until the source reports that
    /// nothing is available.
    pub fn poll(&mut self) -> Result<usize, BridgeError> {
        let mut buf = [0u8; BUF_SIZE];
        let mut fed = 0usize;
        loop {
            let n = self.source.read_available(&mut buf)?;
            if n == 0 {
                break;
            }
            self.last_frame = self.emu.parse_bytes(&buf[..n]);
            fed += n;
        }
        Ok(fed)
    }
}
