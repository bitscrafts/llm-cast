//! The throttled pipeline loop: poll the capture source, rasterize changed
//! frames, submit them to the encoder, and keep a static screen alive with
//! periodic keepalive frames so HLS segments keep flowing (R11).
//!
//! The coordinator is synchronous and generic over `S: ByteSource,
//! E: Encode` — fully testable in-container with an in-memory source and
//! [`NullEncoder`]; `run()` additionally needs a tokio runtime for the
//! shutdown signal (tokio is already "full").

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::capture::bridge::{Bridge, ByteSource};
use crate::encode::pipe::{Encode, EncodeError};
use crate::render::raster::{rasterize, TILE};

/// One throttled pipeline step; injectable `now_ms` makes the cadence
/// testable.
pub struct PipelineConfig {
    /// Push a frame even when idle, to keep HLS segments flowing (default 1000).
    pub keepalive_ms: u64,
    /// Loop granularity (default 10).
    pub tick_ms: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        PipelineConfig {
            keepalive_ms: 1000,
            tick_ms: 10,
        }
    }
}

/// The capture → emu → rasterize → encode loop.
pub struct Pipeline<S: ByteSource, E: Encode> {
    bridge: Bridge<S>,
    encoder: E,
    config: PipelineConfig,
    /// Reusable RGBA canvas (sized from the emu grid × 8).
    buf: Vec<u8>,
    last_submit_ms: u64,
}

impl<S: ByteSource, E: Encode> Pipeline<S, E> {
    /// Wrap `bridge` around `encoder` with the given cadence.
    pub fn new(bridge: Bridge<S>, encoder: E, config: PipelineConfig) -> Self {
        Pipeline {
            bridge,
            encoder,
            config,
            buf: Vec::new(),
            last_submit_ms: 0,
        }
    }

    /// The encoder — lets tests observe what was submitted.
    pub fn encoder(&self) -> &E {
        &self.encoder
    }

    /// One step: poll the source, submit on damage, keepalive when idle.
    pub fn poll_and_submit(&mut self, now_ms: u64) -> Result<(), EncodeError> {
        let fed = self.bridge.poll()?;
        let frame = self.bridge.frame();

        // A changed diff frame has cells; an empty diff (or no new bytes)
        // means the screen is unchanged and only the keepalive can submit.
        let changed = fed > 0 && !frame.cells.is_empty();
        let idle = now_ms.saturating_sub(self.last_submit_ms);
        if !changed && idle < self.config.keepalive_ms {
            return Ok(());
        }

        let px_width = frame.width as usize * TILE;
        let px_height = frame.height as usize * TILE;
        let canvas = px_width * px_height * 4;
        if self.buf.len() != canvas {
            self.buf.resize(canvas, 0);
        }
        rasterize(frame, &mut self.buf);
        self.encoder.submit_frame(&self.buf, px_width, px_height)?;
        self.last_submit_ms = now_ms;
        Ok(())
    }

    /// Loop [`Pipeline::poll_and_submit`] with real time until a shutdown
    /// signal (SIGINT via tokio). Errors are logged and the loop continues —
    /// the pipeline never hangs and never exits on a bad step.
    pub fn run(&mut self) {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                log::warn!("pipeline: cannot create runtime: {e}");
                return;
            }
        };
        // Shutdown signal: resolves when the operator stops mirror (Ctrl-C).
        let shutdown = rt.spawn(async { tokio::signal::ctrl_c().await.is_ok() });

        loop {
            if shutdown.is_finished() {
                break;
            }
            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(d) => d.as_millis() as u64,
                Err(_) => 0,
            };
            if let Err(e) = self.poll_and_submit(now) {
                log::warn!("pipeline: step failed: {e}");
            }
            std::thread::sleep(Duration::from_millis(self.config.tick_ms));
        }
    }
}
