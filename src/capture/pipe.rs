//! PipeSource (R1, part 6): reads a tmux/herdr `pipe-pane` output FILE.
//!
//! The tmux-compatible integration is the operator's
//! `tmux pipe-pane -t <target> <file>` (herdr exposes the same mechanism),
//! which appends raw escape bytes to the file as the pane changes. A
//! regular-file read never blocks and returns 0 at the current EOF, so no
//! `O_NONBLOCK`/`libc` dependency is needed: the `File` cursor advances
//! across polls exactly as the [`ByteSource`] contract pinned in part 2
//! requires (`read_available` copies what is available, never blocks).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::capture::bridge::{BridgeError, ByteSource};

/// Reads a growing pipe-pane output file: whatever bytes are available now.
pub struct PipeSource {
    file: File,
}

impl PipeSource {
    /// Open the pipe-pane output file for reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BridgeError> {
        let file = File::open(path)?;
        Ok(PipeSource { file })
    }
}

impl ByteSource for PipeSource {
    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, BridgeError> {
        // A regular-file read never blocks; 0 means "at the current EOF" —
        // the pane file keeps growing, so later polls advance the cursor.
        Ok(self.file.read(buf)?)
    }
}
