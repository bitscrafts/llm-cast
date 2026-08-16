//! Server error type (spec-03 R10). Every tool, the runner, config and cast
//! seams report through this; it also wraps the mux layer's [`MuxError`].

use crate::mux::MuxError;

/// The single error surface of the MCP server. Tools map "tool ran but failed"
/// into a well-formed `is_error` result (part 2); this type is the source for
/// that mapping and for genuine internal failures.
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    /// A mux-layer failure.
    #[error("mux error: {0}")]
    Mux(#[from] MuxError),
    /// A cast (media load) failure.
    #[error("cast error: {0}")]
    Cast(String),
    /// A subprocess could not be spawned.
    #[error("process error running {argv:?}: {source}")]
    Runner {
        /// The full argv that failed to spawn.
        argv: Vec<String>,
        /// The underlying OS error.
        source: std::io::Error,
    },
    /// Invalid configuration.
    #[error("configuration error: {0}")]
    Config(String),
    /// A tool received an invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// An internal invariant broke.
    #[error("internal error: {0}")]
    Internal(String),
}
